// NBM_R-Native: Native Rust Port of HBM's Nuclear Tech Mod for Horizon.
//
// This file serves as both the standalone Cargo library crate and the
// injected `crate::game::nbm_r` module inside the Horizon engine.

pub fn block_break_message(pos: [i32; 3]) -> String {
    format!(
        "[nbm_r] Breaking block at {}, {}, {}",
        pos[0], pos[1], pos[2]
    )
}

pub fn before_break(pos: [i32; 3]) {
    eprintln!("{}", block_break_message(pos));
}

/// Core radiation simulation logic (pure Rust, testable without game runtime).
pub mod radiation {
    use std::collections::HashMap;

    pub const MAX_CHUNK_RADIATION: f32 = 100_000.0;
    pub const RADIATION_DECAY_RATE: f32 = 0.99;
    pub const RADIATION_DECAY_SUBTRAHEND: f32 = 0.05;

    pub const MAX_CHUNK_FALLOUT: f32 = 100_000.0;
    pub const FALLOUT_DECAY_RATE_DRY: f32 = 0.9995;
    pub const FALLOUT_DECAY_RATE_RAIN: f32 = 0.90;
    pub const FALLOUT_DECAY_SUBTRAHEND: f32 = 0.01;

    pub const DIFFUSION_CENTER_WEIGHT: f32 = 0.60;
    pub const DIFFUSION_CARDINAL_WEIGHT: f32 = 0.075;
    pub const DIFFUSION_DIAGONAL_WEIGHT: f32 = 0.025;

    pub const DIFFUSION_OFFSETS_AND_WEIGHTS: [((i32, i32), f32); 9] = [
        ((0, 0), DIFFUSION_CENTER_WEIGHT),
        ((1, 0), DIFFUSION_CARDINAL_WEIGHT),
        ((-1, 0), DIFFUSION_CARDINAL_WEIGHT),
        ((0, 1), DIFFUSION_CARDINAL_WEIGHT),
        ((0, -1), DIFFUSION_CARDINAL_WEIGHT),
        ((1, 1), DIFFUSION_DIAGONAL_WEIGHT),
        ((-1, 1), DIFFUSION_DIAGONAL_WEIGHT),
        ((1, -1), DIFFUSION_DIAGONAL_WEIGHT),
        ((-1, -1), DIFFUSION_DIAGONAL_WEIGHT),
    ];

    /// Calculate dose attenuation based on shielding thickness and material factor:
    /// I = I_0 * exp(-mu * d)
    pub fn calculate_attenuation(raw_rads: f32, shielding_coeff: f32, thickness_blocks: f32) -> f32 {
        if raw_rads <= 0.0 {
            return 0.0;
        }
        if shielding_coeff <= 0.0 || thickness_blocks <= 0.0 {
            return raw_rads;
        }
        let factor = (-shielding_coeff * thickness_blocks).exp();
        raw_rads * factor
    }

    /// Calculate dose attenuation based on protective resistance:
    /// I = I_0 * 10^(-R)
    pub fn calculate_resistance_attenuation(raw_rads: f32, resistance: f32) -> f32 {
        if raw_rads <= 0.0 {
            return 0.0;
        }
        if resistance <= 0.0 {
            return raw_rads;
        }
        let factor = 10.0_f32.powf(-resistance);
        raw_rads * factor
    }

    /// Calculate hazmat suit attenuation based on equipped piece count (0 to 4).
    /// Each piece provides 0.5 resistance (total 2.0 for full suit = 99% attenuation).
    pub fn calculate_hazmat_attenuation(raw_rads: f32, hazmat_pieces: usize) -> f32 {
        if raw_rads <= 0.0 {
            return 0.0;
        }
        let pieces = hazmat_pieces.min(4) as f32;
        let resistance = pieces * 0.5;
        calculate_resistance_attenuation(raw_rads, resistance)
    }

    /// Decay background chunk radiation per tick step matching Java 1.7.10:
    /// new_rad = clamp((rad * 0.99 - 0.05), 0.0, 100_000.0)
    pub fn decay_chunk_radiation(rad: f32) -> f32 {
        if rad <= 0.0 {
            return 0.0;
        }
        let decayed = rad * RADIATION_DECAY_RATE - RADIATION_DECAY_SUBTRAHEND;
        decayed.clamp(0.0, MAX_CHUNK_RADIATION)
    }

    /// Decay physical fallout per time interval. Raining accelerates fallout deposition/washout.
    pub fn decay_fallout(fallout: f32, is_raining: bool) -> f32 {
        if fallout <= 0.0 {
            return 0.0;
        }
        let rate = if is_raining {
            FALLOUT_DECAY_RATE_RAIN
        } else {
            FALLOUT_DECAY_RATE_DRY
        };
        let decayed = fallout * rate - FALLOUT_DECAY_SUBTRAHEND;
        decayed.clamp(0.0, MAX_CHUNK_FALLOUT)
    }

    /// Return diffusion kernel weight for relative chunk offset (dx, dz).
    pub fn diffusion_kernel_weight(dx: i32, dz: i32) -> f32 {
        let dist = dx.abs() + dz.abs();
        match dist {
            0 => DIFFUSION_CENTER_WEIGHT,
            1 => DIFFUSION_CARDINAL_WEIGHT,
            2 if dx.abs() == 1 && dz.abs() == 1 => DIFFUSION_DIAGONAL_WEIGHT,
            _ => 0.0,
        }
    }

    /// Distribute single chunk radiation to 9 neighboring positions according to diffusion kernel.
    pub fn diffuse_single_chunk(source_rad: f32) -> [((i32, i32), f32); 9] {
        [
            ((0, 0), source_rad * DIFFUSION_CENTER_WEIGHT),
            ((1, 0), source_rad * DIFFUSION_CARDINAL_WEIGHT),
            ((-1, 0), source_rad * DIFFUSION_CARDINAL_WEIGHT),
            ((0, 1), source_rad * DIFFUSION_CARDINAL_WEIGHT),
            ((0, -1), source_rad * DIFFUSION_CARDINAL_WEIGHT),
            ((1, 1), source_rad * DIFFUSION_DIAGONAL_WEIGHT),
            ((-1, 1), source_rad * DIFFUSION_DIAGONAL_WEIGHT),
            ((1, -1), source_rad * DIFFUSION_DIAGONAL_WEIGHT),
            ((-1, -1), source_rad * DIFFUSION_DIAGONAL_WEIGHT),
        ]
    }

    /// Diffuse radiation across an open coordinate map of chunks with exact mass conservation.
    pub fn diffuse_chunk_radiation_map(
        chunks: &HashMap<(i32, i32), f32>,
    ) -> HashMap<(i32, i32), f32> {
        let mut next = HashMap::new();
        for (&(cx, cz), &rad) in chunks {
            if rad <= 0.0 {
                continue;
            }
            for &((dx, dz), weight) in &DIFFUSION_OFFSETS_AND_WEIGHTS {
                let target_coord = (cx + dx, cz + dz);
                *next.entry(target_coord).or_insert(0.0) += rad * weight;
            }
        }
        next
    }

    /// Diffuse and decay radiation across an open chunk map, replicating Java ChunkRadiationHandlerSimple.updateSystem().
    pub fn diffuse_and_decay_chunk_radiation_map(
        chunks: &HashMap<(i32, i32), f32>,
    ) -> HashMap<(i32, i32), f32> {
        let diffused = diffuse_chunk_radiation_map(chunks);
        let mut result = HashMap::with_capacity(diffused.len());
        for (coord, rad) in diffused {
            let decayed = decay_chunk_radiation(rad);
            if decayed > 0.0 {
                result.insert(coord, decayed);
            }
        }
        result
    }

    /// Diffuse radiation across a 2D toroidal grid with exact mass conservation.
    pub fn diffuse_grid_toroidal<const W: usize, const H: usize>(
        grid: &[[f32; W]; H],
    ) -> [[f32; W]; H] {
        let mut next = [[0.0_f32; W]; H];
        for y in 0..H {
            for x in 0..W {
                let val = grid[y][x];
                if val <= 0.0 {
                    continue;
                }
                for &((dx, dz), weight) in &DIFFUSION_OFFSETS_AND_WEIGHTS {
                    let nx = ((x as isize + dx as isize).rem_euclid(W as isize)) as usize;
                    let ny = ((y as isize + dz as isize).rem_euclid(H as isize)) as usize;
                    next[ny][nx] += val * weight;
                }
            }
        }
        next
    }
}

/// Declarative radiological items, blocks, and tags.
pub mod content {
    pub const ID: &str = "nbm_r";

    pub const BLOCK_URANIUM_ORE: &str = "nbm_r:uranium_ore";
    pub const ITEM_URANIUM_INGOT: &str = "nbm_r:uranium_ingot";
    pub const ITEM_GEIGER_COUNTER: &str = "nbm_r:geiger_counter";
    pub const ITEM_HAZMAT_HELMET: &str = "nbm_r:hazmat_helmet";
    pub const ITEM_HAZMAT_CHESTPLATE: &str = "nbm_r:hazmat_chestplate";
    pub const ITEM_HAZMAT_LEGGINGS: &str = "nbm_r:hazmat_leggings";
    pub const ITEM_HAZMAT_BOOTS: &str = "nbm_r:hazmat_boots";

    pub const TAG_HAZMAT: &str = "#ntm:hazmat";
    pub const TAG_RADIOACTIVE: &str = "#ntm:radioactive";

    /// Default ionizing radiation emission rate for raw Uranium Ingot in Rad/s.
    pub const URANIUM_INGOT_RAD_PER_SEC: f32 = 1.5;
    /// Default ionizing radiation emission rate for Uranium Ore block in Rad/s.
    pub const URANIUM_ORE_RAD_PER_SEC: f32 = 0.5;

    /// Uranium ingot stack size limit.
    pub const URANIUM_INGOT_MAX_STACK: u8 = 64;
    /// Geiger counter stack size limit.
    pub const GEIGER_COUNTER_MAX_STACK: u8 = 1;
    /// Hazmat suit pieces stack size limit.
    pub const HAZMAT_PIECE_MAX_STACK: u8 = 1;

    /// Full hazmat suit pieces list.
    pub const HAZMAT_PIECES: [&str; 4] = [
        ITEM_HAZMAT_HELMET,
        ITEM_HAZMAT_CHESTPLATE,
        ITEM_HAZMAT_LEGGINGS,
        ITEM_HAZMAT_BOOTS,
    ];

    /// Check if item id is a registered Hazmat armor piece.
    pub fn is_hazmat_piece(item_id: &str) -> bool {
        HAZMAT_PIECES.contains(&item_id)
            || item_id == "ntm:hazmat_helmet"
            || item_id == "ntm:hazmat_chestplate"
            || item_id == "ntm:hazmat_leggings"
            || item_id == "ntm:hazmat_boots"
    }

    /// Return radiation emission rate for radioactive items in Rad/s.
    pub fn item_radiation_emission(item_id: &str) -> f32 {
        if item_id == ITEM_URANIUM_INGOT || item_id == "ntm:ingot_uranium" || item_id == "ntm:uranium_ingot" {
            URANIUM_INGOT_RAD_PER_SEC
        } else if item_id == BLOCK_URANIUM_ORE || item_id == "ntm:ore_uranium" || item_id == "ntm:uranium_ore" {
            URANIUM_ORE_RAD_PER_SEC
        } else {
            0.0
        }
    }
}

pub const ID: &str = content::ID;

/// World chunk tracking and player dosimetry simulation.
pub mod dosimetry {
    use std::collections::{HashMap, HashSet};
    use super::content;
    use super::radiation;
    use super::sickness;

    /// Chunk radiation state storing ambient Rad and particulate Fallout.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct ChunkRadiationData {
        pub rad: f32,
        pub fallout: f32,
    }

    impl ChunkRadiationData {
        pub const fn new(rad: f32, fallout: f32) -> Self {
            Self { rad, fallout }
        }

        pub fn is_empty(&self) -> bool {
            self.rad <= 0.0 && self.fallout <= 0.0
        }

        /// 8-byte little-endian binary serialization: [rad: 4 bytes, fallout: 4 bytes].
        pub fn to_bytes(&self) -> [u8; 8] {
            let mut bytes = [0u8; 8];
            bytes[0..4].copy_from_slice(&self.rad.to_le_bytes());
            bytes[4..8].copy_from_slice(&self.fallout.to_le_bytes());
            bytes
        }

        /// Deserialize from 8-byte little-endian slice.
        pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
            if bytes.len() < 8 {
                return None;
            }
            let rad = f32::from_le_bytes(bytes[0..4].try_into().ok()?);
            let fallout = f32::from_le_bytes(bytes[4..8].try_into().ok()?);
            Some(Self { rad, fallout })
        }

        /// Deterministic JSON format for human-readable debugging / storage.
        pub fn to_json(&self) -> String {
            format!(r#"{{"rad":{:.4},"fallout":{:.4}}}"#, self.rad, self.fallout)
        }

        /// Parse from JSON string.
        pub fn from_json(s: &str) -> Option<Self> {
            let s = s.trim();
            let rad_idx = s.find("\"rad\":")?;
            let after_rad = &s[rad_idx + 6..];
            let rad_end = after_rad.find(|c: char| c == ',' || c == '}')?;
            let rad: f32 = after_rad[..rad_end].trim().parse().ok()?;

            let fallout_idx = s.find("\"fallout\":")?;
            let after_fallout = &s[fallout_idx + 10..];
            let fallout_end = after_fallout.find(|c: char| c == ',' || c == '}')?;
            let fallout: f32 = after_fallout[..fallout_end].trim().parse().ok()?;

            Some(Self { rad, fallout })
        }
    }

    /// Player absorbed dose tracking.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct PlayerDosimetryData {
        /// Accumulated absorbed dose in Rad.
        pub dose: f32,
        /// Last measured ambient dose rate at player position (Rad/s).
        pub last_ambient_rate: f32,
        /// Last measured absorbed dose rate after armor attenuation (Rad/s).
        pub last_absorbed_rate: f32,
        /// Number of exposure ticks processed for this player.
        pub ticks_alive: u64,
    }

    impl PlayerDosimetryData {
        pub const fn new(dose: f32) -> Self {
            Self {
                dose,
                last_ambient_rate: 0.0,
                last_absorbed_rate: 0.0,
                ticks_alive: 0,
            }
        }

        pub fn is_empty(&self) -> bool {
            self.dose <= 0.0 && self.last_ambient_rate <= 0.0 && self.last_absorbed_rate <= 0.0
        }

        /// 20-byte little-endian binary serialization:
        /// [dose: 4B, last_ambient_rate: 4B, last_absorbed_rate: 4B, ticks_alive: 8B]
        pub fn to_bytes(&self) -> [u8; 20] {
            let mut bytes = [0u8; 20];
            bytes[0..4].copy_from_slice(&self.dose.to_le_bytes());
            bytes[4..8].copy_from_slice(&self.last_ambient_rate.to_le_bytes());
            bytes[8..12].copy_from_slice(&self.last_absorbed_rate.to_le_bytes());
            bytes[12..20].copy_from_slice(&self.ticks_alive.to_le_bytes());
            bytes
        }

        /// Deserialize from little-endian slice (accepts 20 bytes, or 12 bytes backwards-compatible).
        pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
            if bytes.len() < 12 {
                return None;
            }
            let dose = f32::from_le_bytes(bytes[0..4].try_into().ok()?);
            let last_ambient_rate = f32::from_le_bytes(bytes[4..8].try_into().ok()?);
            let last_absorbed_rate = f32::from_le_bytes(bytes[8..12].try_into().ok()?);
            let ticks_alive = if bytes.len() >= 20 {
                u64::from_le_bytes(bytes[12..20].try_into().ok()?)
            } else {
                0
            };
            Some(Self {
                dose,
                last_ambient_rate,
                last_absorbed_rate,
                ticks_alive,
            })
        }

        /// Deterministic JSON format for human-readable debugging / storage.
        pub fn to_json(&self) -> String {
            format!(
                r#"{{"dose":{:.4},"ambient_rate":{:.4},"absorbed_rate":{:.4},"ticks":{}}}"#,
                self.dose, self.last_ambient_rate, self.last_absorbed_rate, self.ticks_alive
            )
        }

        /// Parse from JSON string.
        pub fn from_json(s: &str) -> Option<Self> {
            let s = s.trim();
            let dose_idx = s.find("\"dose\":")?;
            let after_dose = &s[dose_idx + 7..];
            let dose_end = after_dose.find(|c: char| c == ',' || c == '}')?;
            let dose: f32 = after_dose[..dose_end].trim().parse().ok()?;

            let amb_idx = s.find("\"ambient_rate\":")?;
            let after_amb = &s[amb_idx + 15..];
            let amb_end = after_amb.find(|c: char| c == ',' || c == '}')?;
            let last_ambient_rate: f32 = after_amb[..amb_end].trim().parse().ok()?;

            let abs_idx = s.find("\"absorbed_rate\":")?;
            let after_abs = &s[abs_idx + 16..];
            let abs_end = after_abs.find(|c: char| c == ',' || c == '}')?;
            let last_absorbed_rate: f32 = after_abs[..abs_end].trim().parse().ok()?;

            let ticks = if let Some(ticks_idx) = s.find("\"ticks\":") {
                let after_ticks = &s[ticks_idx + 8..];
                let ticks_end = after_ticks.find(|c: char| c == ',' || c == '}')?;
                after_ticks[..ticks_end].trim().parse().ok()?
            } else {
                0
            };

            Some(Self {
                dose,
                last_ambient_rate,
                last_absorbed_rate,
                ticks_alive: ticks,
            })
        }
    }

    /// World-level radiation tracking across dimensions and chunks.
    #[derive(Clone, Debug, Default)]
    pub struct WorldRadiationTracker {
        /// Chunks currently loaded by dimension and chunk coordinate (cx, cz).
        pub loaded_chunks: HashMap<String, HashSet<(i32, i32)>>,
        /// Chunk radiation storage per dimension and chunk coordinate (cx, cz).
        pub chunk_data: HashMap<(String, i32, i32), ChunkRadiationData>,
    }

    impl WorldRadiationTracker {
        pub fn new() -> Self {
            Self::default()
        }

        /// Register chunk as loaded.
        pub fn on_chunk_load(&mut self, dimension: &str, cx: i32, cz: i32) {
            self.loaded_chunks
                .entry(dimension.to_owned())
                .or_default()
                .insert((cx, cz));
        }

        /// Register chunk as unloaded.
        pub fn on_chunk_unload(&mut self, dimension: &str, cx: i32, cz: i32) {
            if let Some(set) = self.loaded_chunks.get_mut(dimension) {
                set.remove(&(cx, cz));
            }
        }

        /// Check if a chunk is currently loaded.
        pub fn is_chunk_loaded(&self, dimension: &str, cx: i32, cz: i32) -> bool {
            self.loaded_chunks
                .get(dimension)
                .map_or(false, |set| set.contains(&(cx, cz)))
        }

        /// Get radiation data for a chunk.
        pub fn get_chunk_data(&self, dimension: &str, cx: i32, cz: i32) -> ChunkRadiationData {
            self.chunk_data
                .get(&(dimension.to_owned(), cx, cz))
                .copied()
                .unwrap_or_default()
        }

        /// Set radiation data for a chunk.
        pub fn set_chunk_data(
            &mut self,
            dimension: &str,
            cx: i32,
            cz: i32,
            data: ChunkRadiationData,
        ) {
            let key = (dimension.to_owned(), cx, cz);
            if data.is_empty() {
                self.chunk_data.remove(&key);
            } else {
                self.chunk_data.insert(key, data);
            }
        }

        /// Add radiation to a specific chunk.
        pub fn add_radiation(&mut self, dimension: &str, cx: i32, cz: i32, amount: f32) {
            if amount <= 0.0 {
                return;
            }
            let key = (dimension.to_owned(), cx, cz);
            let entry = self.chunk_data.entry(key).or_default();
            entry.rad = (entry.rad + amount).clamp(0.0, radiation::MAX_CHUNK_RADIATION);
        }

        /// Add fallout to a specific chunk.
        pub fn add_fallout(&mut self, dimension: &str, cx: i32, cz: i32, amount: f32) {
            if amount <= 0.0 {
                return;
            }
            let key = (dimension.to_owned(), cx, cz);
            let entry = self.chunk_data.entry(key).or_default();
            entry.fallout = (entry.fallout + amount).clamp(0.0, radiation::MAX_CHUNK_FALLOUT);
        }

        /// Execute a post-level tick step for a dimension:
        /// 1. Collects all chunks in this dimension with radiation > 0.
        /// 2. Performs neighbor diffusion and decay via pure math engine `diffuse_and_decay_chunk_radiation_map`.
        /// 3. Performs fallout decay via `decay_fallout`.
        /// 4. Updates active irradiated chunks in storage.
        pub fn tick_level(&mut self, dimension: &str, is_raining: bool) {
            let mut active_rad_map = HashMap::new();
            for (&(ref dim, cx, cz), data) in &self.chunk_data {
                if dim == dimension && data.rad > 0.0 {
                    active_rad_map.insert((cx, cz), data.rad);
                }
            }

            let next_rad_map = if !active_rad_map.is_empty() {
                radiation::diffuse_and_decay_chunk_radiation_map(&active_rad_map)
            } else {
                HashMap::new()
            };

            let mut coords: HashSet<(i32, i32)> = HashSet::new();
            for &(ref dim, cx, cz) in self.chunk_data.keys() {
                if dim == dimension {
                    coords.insert((cx, cz));
                }
            }
            for &coord in next_rad_map.keys() {
                coords.insert(coord);
            }

            for (cx, cz) in coords {
                let key = (dimension.to_owned(), cx, cz);
                let current_fallout = self.chunk_data.get(&key).map_or(0.0, |d| d.fallout);
                let new_rad = next_rad_map.get(&(cx, cz)).copied().unwrap_or(0.0);
                let new_fallout = radiation::decay_fallout(current_fallout, is_raining);

                if new_rad <= 0.0 && new_fallout <= 0.0 {
                    self.chunk_data.remove(&key);
                } else {
                    self.chunk_data.insert(
                        key,
                        ChunkRadiationData {
                            rad: new_rad,
                            fallout: new_fallout,
                        },
                    );
                }
            }
        }
    }

    /// Calculate total ambient dose rate (Rad/s) at player position:
    /// ambient_chunk_rad + inventory items emission.
    pub fn calculate_ambient_exposure(
        chunk_rad: f32,
        inventory_items: &[(&str, u32)],
    ) -> f32 {
        let mut total = chunk_rad.max(0.0);
        for &(item_id, count) in inventory_items {
            let emission = content::item_radiation_emission(item_id);
            if emission > 0.0 && count > 0 {
                total += emission * count as f32;
            }
        }
        total
    }

    /// Calculate absorbed dose rate (Rad/s) after hazmat armor attenuation.
    /// Wearing 4 hazmat pieces reduces dose by 99% (multiplier 0.01).
    pub fn calculate_absorbed_exposure(ambient_rad_rate: f32, hazmat_pieces: usize) -> f32 {
        radiation::calculate_hazmat_attenuation(ambient_rad_rate, hazmat_pieces)
    }

    /// Advance dose accumulation for a player over `delta_seconds` (e.g. 0.05 for 1 tick).
    pub fn step_player_dosimetry(
        dosimetry: &mut PlayerDosimetryData,
        chunk_rad: f32,
        inventory_items: &[(&str, u32)],
        hazmat_pieces: usize,
        delta_seconds: f32,
    ) -> f32 {
        let ambient_rate = calculate_ambient_exposure(chunk_rad, inventory_items);
        let absorbed_rate = calculate_absorbed_exposure(ambient_rate, hazmat_pieces);
        dosimetry.last_ambient_rate = ambient_rate;
        dosimetry.last_absorbed_rate = absorbed_rate;
        if ambient_rate > 0.0 {
            dosimetry.dose += absorbed_rate * delta_seconds;
        } else {
            dosimetry.dose = sickness::calculate_natural_recovery(
                dosimetry.dose,
                ambient_rate,
                delta_seconds,
            );
        }
        dosimetry.ticks_alive = dosimetry.ticks_alive.saturating_add(1);
        absorbed_rate
    }

    /// Convert block world coordinate to chunk coordinate (cx, cz).
    pub fn block_to_chunk(x: i32, z: i32) -> (i32, i32) {
        (x.div_euclid(16), z.div_euclid(16))
    }

    /// Convert entity continuous coordinates to chunk coordinate (cx, cz).
    pub fn pos_to_chunk(x: f64, z: f64) -> (i32, i32) {
        let ix = x.floor() as i32;
        let iz = z.floor() as i32;
        (ix.div_euclid(16), iz.div_euclid(16))
    }
}

/// Biological radiation sickness thresholds, status effects, and lethal damage logic.
pub mod sickness {
    pub const THRESHOLD_MILD: f32 = 200.0;
    pub const THRESHOLD_SEVERE: f32 = 500.0;
    pub const THRESHOLD_LETHAL: f32 = 1000.0;

    pub const DAMAGE_TYPE_RADIATION: &str = "ntm:radiation";
    pub const NATURAL_RECOVERY_RATE: f32 = 0.05; // 0.05 Rad/s in clean environment
    pub const LETHAL_DAMAGE_AMOUNT: f32 = 1000.0;
    pub const SEVERE_PERIODIC_DAMAGE: f32 = 1.0;
    pub const SEVERE_DAMAGE_INTERVAL_TICKS: u64 = 100; // 5 seconds (20 ticks/sec * 5)

    pub const EFFECT_NAUSEA: &str = "minecraft:nausea";
    pub const EFFECT_SLOWNESS: &str = "minecraft:slowness";
    pub const EFFECT_WEAKNESS: &str = "minecraft:weakness";
    pub const EFFECT_MINING_FATIGUE: &str = "minecraft:mining_fatigue";

    /// Progressive biological sickness stages based on accumulated Rad dose.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum RadiationSicknessStage {
        None,
        Mild,
        Severe,
        Lethal,
    }

    impl RadiationSicknessStage {
        pub fn from_dose(dose: f32) -> Self {
            if dose >= THRESHOLD_LETHAL {
                Self::Lethal
            } else if dose >= THRESHOLD_SEVERE {
                Self::Severe
            } else if dose >= THRESHOLD_MILD {
                Self::Mild
            } else {
                Self::None
            }
        }

        pub fn is_sick(&self) -> bool {
            matches!(self, Self::Mild | Self::Severe | Self::Lethal)
        }
    }

    /// Status effect descriptor inflicted by radiation sickness.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct StatusEffect {
        pub id: &'static str,
        pub amplifier: u8,
        pub duration_ticks: i32,
    }

    /// Retrieve the status effects inflicted by a given sickness stage.
    pub fn get_stage_effects(stage: RadiationSicknessStage) -> Vec<StatusEffect> {
        match stage {
            RadiationSicknessStage::None => Vec::new(),
            RadiationSicknessStage::Mild => vec![
                StatusEffect {
                    id: EFFECT_NAUSEA,
                    amplifier: 0,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_SLOWNESS,
                    amplifier: 0,
                    duration_ticks: 100,
                },
            ],
            RadiationSicknessStage::Severe => vec![
                StatusEffect {
                    id: EFFECT_NAUSEA,
                    amplifier: 0,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_SLOWNESS,
                    amplifier: 1,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_WEAKNESS,
                    amplifier: 0,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_MINING_FATIGUE,
                    amplifier: 0,
                    duration_ticks: 100,
                },
            ],
            RadiationSicknessStage::Lethal => vec![
                StatusEffect {
                    id: EFFECT_NAUSEA,
                    amplifier: 0,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_SLOWNESS,
                    amplifier: 2,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_WEAKNESS,
                    amplifier: 1,
                    duration_ticks: 100,
                },
                StatusEffect {
                    id: EFFECT_MINING_FATIGUE,
                    amplifier: 1,
                    duration_ticks: 100,
                },
            ],
        }
    }

    /// Evaluation result of radiation sickness on an entity.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SicknessEvaluation {
        pub stage: RadiationSicknessStage,
        pub effects: Vec<StatusEffect>,
        pub damage: Option<f32>,
        pub damage_type: Option<&'static str>,
    }

    /// Evaluate biological radiation sickness effects and damage for a given dose and tick.
    pub fn evaluate_sickness(dose: f32, ticks: u64) -> SicknessEvaluation {
        let stage = RadiationSicknessStage::from_dose(dose);
        let effects = get_stage_effects(stage);
        let (damage, damage_type) = match stage {
            RadiationSicknessStage::Lethal => (
                Some(LETHAL_DAMAGE_AMOUNT),
                Some(DAMAGE_TYPE_RADIATION),
            ),
            RadiationSicknessStage::Severe => {
                if ticks % SEVERE_DAMAGE_INTERVAL_TICKS == 0 {
                    (
                        Some(SEVERE_PERIODIC_DAMAGE),
                        Some(DAMAGE_TYPE_RADIATION),
                    )
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };
        SicknessEvaluation {
            stage,
            effects,
            damage,
            damage_type,
        }
    }

    /// Calculate natural biological dose recovery in clean (zero-radiation) environment.
    /// When ambient radiation is 0 (or <= 0), recovers by 0.05 Rad/s.
    /// When exposed to radiation (ambient > 0), no natural recovery occurs.
    pub fn calculate_natural_recovery(
        current_dose: f32,
        ambient_rate: f32,
        delta_seconds: f32,
    ) -> f32 {
        if ambient_rate <= 0.0 {
            (current_dose - NATURAL_RECOVERY_RATE * delta_seconds).max(0.0)
        } else {
            current_dose
        }
    }

    /// Check if a damage source is custom ntm:radiation damage.
    pub fn is_radiation_damage(damage_type: &str) -> bool {
        damage_type == DAMAGE_TYPE_RADIATION
    }
}

// Module-level thread-safe state trackers
use std::collections::HashMap;
use std::sync::Mutex;

static WORLD_TRACKER: Mutex<Option<dosimetry::WorldRadiationTracker>> = Mutex::new(None);
static PLAYER_DOSES: Mutex<Option<HashMap<u128, dosimetry::PlayerDosimetryData>>> = Mutex::new(None);

pub fn lock_tracker<F, R>(f: F) -> R
where
    F: FnOnce(&mut dosimetry::WorldRadiationTracker) -> R,
{
    let mut guard = WORLD_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    let tracker = guard.get_or_insert_with(dosimetry::WorldRadiationTracker::new);
    f(tracker)
}

pub fn lock_players<F, R>(f: F) -> R
where
    F: FnOnce(&mut HashMap<u128, dosimetry::PlayerDosimetryData>) -> R,
{
    let mut guard = PLAYER_DOSES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

/// Reset world radiation and player dosimetry state (useful for tests and level unloading).
pub fn reset_state() {
    let mut w = WORLD_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    *w = None;
    let mut p = PLAYER_DOSES.lock().unwrap_or_else(|e| e.into_inner());
    *p = None;
}

pub fn on_chunk_load(dimension: &str, chunk: [i32; 2]) {
    lock_tracker(|t| t.on_chunk_load(dimension, chunk[0], chunk[1]));
}

pub fn on_chunk_unload(dimension: &str, chunk: [i32; 2]) {
    lock_tracker(|t| t.on_chunk_unload(dimension, chunk[0], chunk[1]));
}

pub fn on_level_tick(dimension: &str) {
    lock_tracker(|t| t.tick_level(dimension, false));
}

pub fn get_chunk_radiation(dimension: &str, cx: i32, cz: i32) -> f32 {
    lock_tracker(|t| t.get_chunk_data(dimension, cx, cz).rad)
}

pub fn set_chunk_radiation(dimension: &str, cx: i32, cz: i32, rad: f32) {
    lock_tracker(|t| {
        let mut data = t.get_chunk_data(dimension, cx, cz);
        data.rad = rad;
        t.set_chunk_data(dimension, cx, cz, data);
    });
}

pub fn get_chunk_data(dimension: &str, cx: i32, cz: i32) -> dosimetry::ChunkRadiationData {
    lock_tracker(|t| t.get_chunk_data(dimension, cx, cz))
}

pub fn set_chunk_data(dimension: &str, cx: i32, cz: i32, data: dosimetry::ChunkRadiationData) {
    lock_tracker(|t| t.set_chunk_data(dimension, cx, cz, data));
}

pub fn get_player_dosimetry(uuid: u128) -> dosimetry::PlayerDosimetryData {
    lock_players(|m| m.get(&uuid).copied().unwrap_or_default())
}

pub fn set_player_dose(uuid: u128, dose: f32) {
    lock_players(|m| {
        let entry = m.entry(uuid).or_default();
        entry.dose = dose;
    });
}

pub fn record_player_exposure(
    uuid: u128,
    chunk_rad: f32,
    inventory_items: &[(&str, u32)],
    hazmat_pieces: usize,
    delta_seconds: f32,
) -> f32 {
    lock_players(|m| {
        let entry = m.entry(uuid).or_default();
        dosimetry::step_player_dosimetry(
            entry,
            chunk_rad,
            inventory_items,
            hazmat_pieces,
            delta_seconds,
        )
    })
}

pub fn tick_player_sickness(uuid: u128) -> sickness::SicknessEvaluation {
    lock_players(|m| {
        let entry = m.entry(uuid).or_default();
        let eval = sickness::evaluate_sickness(entry.dose, entry.ticks_alive);
        if eval.stage == sickness::RadiationSicknessStage::Lethal {
            entry.dose = 0.0;
        }
        eval
    })
}

pub fn on_player_death(uuid: u128) {
    lock_players(|m| {
        if let Some(entry) = m.get_mut(&uuid) {
            entry.dose = 0.0;
            entry.ticks_alive = 0;
        }
    });
}

pub fn save_player_dosimetry(uuid: u128) -> [u8; 20] {
    get_player_dosimetry(uuid).to_bytes()
}

pub fn load_player_dosimetry(uuid: u128, bytes: &[u8]) -> bool {
    if let Some(data) = dosimetry::PlayerDosimetryData::from_bytes(bytes) {
        lock_players(|m| {
            m.insert(uuid, data);
        });
        true
    } else {
        false
    }
}

pub fn save_player_dosimetry_json(uuid: u128) -> String {
    get_player_dosimetry(uuid).to_json()
}

pub fn load_player_dosimetry_json(uuid: u128, json: &str) -> bool {
    if let Some(data) = dosimetry::PlayerDosimetryData::from_json(json) {
        lock_players(|m| {
            m.insert(uuid, data);
        });
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const EPSILON: f32 = 1e-4;

    #[test]
    fn formats_break_message() {
        assert_eq!(
            block_break_message([-3, 64, 12]),
            "[nbm_r] Breaking block at -3, 64, 12"
        );
    }

    #[test]
    fn test_shielding_attenuation_exponential_parity() {
        // I = I_0 * exp(-mu * d)
        // For raw = 100.0, mu = 0.5, d = 2.0 -> 100.0 * exp(-1.0) = 36.787944117
        let shielded = radiation::calculate_attenuation(100.0, 0.5, 2.0);
        assert!((shielded - 36.787944).abs() < EPSILON);

        // For raw = 500.0, mu = 0.6, d = 3.0 -> 500.0 * exp(-1.8) = 82.649445 in IEEE 754 float
        let shielded_lead = radiation::calculate_attenuation(500.0, 0.6, 3.0);
        let expected_lead = 500.0_f32 * (-1.8_f32).exp();
        assert!((shielded_lead - expected_lead).abs() < EPSILON);
        assert!((shielded_lead - 82.649445).abs() < EPSILON);

        // Edge cases
        assert_eq!(radiation::calculate_attenuation(0.0, 0.5, 2.0), 0.0);
        assert_eq!(radiation::calculate_attenuation(-50.0, 0.5, 2.0), 0.0);
        assert_eq!(radiation::calculate_attenuation(100.0, 0.0, 2.0), 100.0);
        assert_eq!(radiation::calculate_attenuation(100.0, 0.5, 0.0), 100.0);
        assert_eq!(radiation::calculate_attenuation(100.0, -0.5, 2.0), 100.0);
        assert_eq!(radiation::calculate_attenuation(100.0, 0.5, -2.0), 100.0);
    }

    #[test]
    fn test_resistance_and_hazmat_attenuation() {
        // Resistance: I = I_0 * 10^(-R)
        // Full hazmat: 4 pieces * 0.5 = 2.0 resistance -> 10^(-2) = 0.01 (99% attenuation)
        let full_hazmat = radiation::calculate_hazmat_attenuation(1000.0, 4);
        assert!((full_hazmat - 10.0).abs() < EPSILON);

        // 2 pieces: 2 * 0.5 = 1.0 resistance -> 10^(-1) = 0.10 (90% attenuation)
        let half_hazmat = radiation::calculate_hazmat_attenuation(1000.0, 2);
        assert!((half_hazmat - 100.0).abs() < EPSILON);

        // 1 piece: 1 * 0.5 = 0.5 resistance -> 10^(-0.5) = 0.3162277
        let single_piece = radiation::calculate_hazmat_attenuation(1000.0, 1);
        assert!((single_piece - 316.227766).abs() < EPSILON);

        // 0 pieces -> 1000.0
        let no_hazmat = radiation::calculate_hazmat_attenuation(1000.0, 0);
        assert!((no_hazmat - 1000.0).abs() < EPSILON);

        // Clamped at 4 pieces max
        let extra_hazmat = radiation::calculate_hazmat_attenuation(1000.0, 6);
        assert!((extra_hazmat - 10.0).abs() < EPSILON);

        // Negative raw rads
        assert_eq!(radiation::calculate_hazmat_attenuation(-50.0, 4), 0.0);
    }

    #[test]
    fn test_environmental_decay_chunk_radiation() {
        // Single decay step: (100.0 * 0.99 - 0.05) = 98.95
        let step1 = radiation::decay_chunk_radiation(100.0);
        assert!((step1 - 98.95).abs() < EPSILON);

        // Low radiation drops to 0: 0.05 * 0.99 - 0.05 = -0.0005 -> 0.0
        let zeroed = radiation::decay_chunk_radiation(0.05);
        assert_eq!(zeroed, 0.0);

        // 0 or negative
        assert_eq!(radiation::decay_chunk_radiation(0.0), 0.0);
        assert_eq!(radiation::decay_chunk_radiation(-10.0), 0.0);

        // Max clamp
        let clamped = radiation::decay_chunk_radiation(200_000.0);
        assert_eq!(clamped, radiation::MAX_CHUNK_RADIATION);

        // Multi-step numerical parity against Java 1.7.10 loop
        let mut java_rad = 1000.0_f32;
        let mut rust_rad = 1000.0_f32;
        for _ in 0..50 {
            // Java calculation:
            java_rad = (java_rad * 0.99 - 0.05).clamp(0.0, 100_000.0);
            rust_rad = radiation::decay_chunk_radiation(rust_rad);
            assert!((rust_rad - java_rad).abs() < EPSILON);
        }
    }

    #[test]
    fn test_fallout_decay() {
        // Dry decay: 100.0 * 0.9995 - 0.01 = 99.94
        let dry = radiation::decay_fallout(100.0, false);
        assert!((dry - 99.94).abs() < EPSILON);

        // Rain decay: 100.0 * 0.90 - 0.01 = 89.99
        let rain = radiation::decay_fallout(100.0, true);
        assert!((rain - 89.99).abs() < EPSILON);

        // Zero / negative clamp
        assert_eq!(radiation::decay_fallout(0.005, false), 0.0);
        assert_eq!(radiation::decay_fallout(0.0, false), 0.0);
        assert_eq!(radiation::decay_fallout(-10.0, true), 0.0);
    }

    #[test]
    fn test_diffusion_kernel_weights() {
        // Kernel weights must sum to exactly 1.0
        let center = radiation::diffusion_kernel_weight(0, 0);
        assert_eq!(center, 0.60);

        let cardinal = [
            radiation::diffusion_kernel_weight(1, 0),
            radiation::diffusion_kernel_weight(-1, 0),
            radiation::diffusion_kernel_weight(0, 1),
            radiation::diffusion_kernel_weight(0, -1),
        ];
        for &w in &cardinal {
            assert_eq!(w, 0.075);
        }

        let diagonal = [
            radiation::diffusion_kernel_weight(1, 1),
            radiation::diffusion_kernel_weight(-1, 1),
            radiation::diffusion_kernel_weight(1, -1),
            radiation::diffusion_kernel_weight(-1, -1),
        ];
        for &w in &diagonal {
            assert_eq!(w, 0.025);
        }

        // Out of kernel range
        assert_eq!(radiation::diffusion_kernel_weight(2, 0), 0.0);
        assert_eq!(radiation::diffusion_kernel_weight(0, -2), 0.0);

        let total_sum = center + cardinal.iter().sum::<f32>() + diagonal.iter().sum::<f32>();
        assert!((total_sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_single_chunk_diffusion_conservation() {
        let initial_mass = 5000.0_f32;
        let diffused = radiation::diffuse_single_chunk(initial_mass);

        let mut sum = 0.0_f32;
        for (_, amount) in diffused {
            sum += amount;
        }
        assert!((sum - initial_mass).abs() < EPSILON);
    }

    #[test]
    fn test_chunk_map_diffusion_exact_mass_conservation() {
        let mut chunks = HashMap::new();
        chunks.insert((0, 0), 1250.0);
        chunks.insert((5, 3), 3400.0);
        chunks.insert((-2, 7), 890.0);

        let initial_total: f32 = chunks.values().sum();

        // Single step: exact conservation within epsilon
        let step1 = radiation::diffuse_chunk_radiation_map(&chunks);
        let total_step1: f32 = step1.values().sum();
        assert!((total_step1 - initial_total).abs() < EPSILON);

        // Multi-step: relative floating point precision conservation < 1e-5
        let mut current = chunks;
        for _ in 0..10 {
            current = radiation::diffuse_chunk_radiation_map(&current);
            let total: f32 = current.values().sum();
            let relative_error = (total - initial_total).abs() / initial_total;
            assert!(
                relative_error < 1e-5,
                "Relative mass error exceeded 1e-5: initial {}, current {}, rel {}",
                initial_total,
                total,
                relative_error
            );
        }
    }

    #[test]
    fn test_grid_diffusion_toroidal_mass_conservation() {
        let mut grid = [[0.0_f32; 8]; 8];
        grid[2][2] = 1000.0;
        grid[0][0] = 500.0; // corner chunk to test wrap-around
        grid[7][7] = 750.0;

        let initial_total: f32 = grid.iter().flat_map(|row| row.iter()).sum();

        // Single step: exact conservation within epsilon
        let step1 = radiation::diffuse_grid_toroidal(&grid);
        let total_step1: f32 = step1.iter().flat_map(|row| row.iter()).sum();
        assert!((total_step1 - initial_total).abs() < EPSILON);

        // Multi-step: relative floating point precision conservation < 1e-5
        let mut current = grid;
        for _ in 0..10 {
            current = radiation::diffuse_grid_toroidal(&current);
            let total: f32 = current.iter().flat_map(|row| row.iter()).sum();
            let relative_error = (total - initial_total).abs() / initial_total;
            assert!(
                relative_error < 1e-5,
                "Toroidal grid relative mass error: initial {}, current {}, rel {}",
                initial_total,
                total,
                relative_error
            );
        }
    }

    #[test]
    fn test_diffuse_and_decay_chunk_radiation_parity() {
        // Java ChunkRadiationHandlerSimple:
        // for each chunk in buff:
        //   diffuse to 9 neighbors
        //   newRad = clamp(newRad * 0.99 - 0.05, 0.0, 100_000.0)
        let mut chunks = HashMap::new();
        chunks.insert((0, 0), 100.0);

        let next_step = radiation::diffuse_and_decay_chunk_radiation_map(&chunks);

        // Center: 100.0 * 0.60 = 60.0 -> decayed: 60.0 * 0.99 - 0.05 = 59.35
        let center = next_step.get(&(0, 0)).copied().unwrap_or(0.0);
        assert!((center - 59.35).abs() < EPSILON);

        // Cardinal: 100.0 * 0.075 = 7.5 -> decayed: 7.5 * 0.99 - 0.05 = 7.375
        let cardinal = next_step.get(&(1, 0)).copied().unwrap_or(0.0);
        assert!((cardinal - 7.375).abs() < EPSILON);

        // Diagonal: 100.0 * 0.025 = 2.5 -> decayed: 2.5 * 0.99 - 0.05 = 2.425
        let diagonal = next_step.get(&(1, 1)).copied().unwrap_or(0.0);
        assert!((diagonal - 2.425).abs() < EPSILON);
    }

    #[test]
    fn test_declarative_content_properties() {
        assert_eq!(content::HAZMAT_PIECES.len(), 4);
        for piece in &content::HAZMAT_PIECES {
            assert!(content::is_hazmat_piece(piece));
        }
        assert!(content::is_hazmat_piece("ntm:hazmat_helmet"));
        assert!(!content::is_hazmat_piece("minecraft:iron_helmet"));

        assert_eq!(content::item_radiation_emission(content::ITEM_URANIUM_INGOT), 1.5);
        assert_eq!(content::item_radiation_emission("ntm:ingot_uranium"), 1.5);
        assert_eq!(content::item_radiation_emission(content::BLOCK_URANIUM_ORE), 0.5);
        assert_eq!(content::item_radiation_emission("minecraft:stone"), 0.0);

        assert_eq!(content::URANIUM_INGOT_MAX_STACK, 64);
        assert_eq!(content::GEIGER_COUNTER_MAX_STACK, 1);
        assert_eq!(content::HAZMAT_PIECE_MAX_STACK, 1);
    }

    #[test]
    fn test_chunk_radiation_data_serialization() {
        let original = dosimetry::ChunkRadiationData::new(123.456, 78.901);
        assert!(!original.is_empty());

        // Binary 8-byte round trip
        let bytes = original.to_bytes();
        assert_eq!(bytes.len(), 8);
        let restored = dosimetry::ChunkRadiationData::from_bytes(&bytes).expect("valid bytes");
        assert!((restored.rad - original.rad).abs() < EPSILON);
        assert!((restored.fallout - original.fallout).abs() < EPSILON);

        // JSON round trip
        let json_str = original.to_json();
        assert!(json_str.contains("\"rad\":"));
        assert!(json_str.contains("\"fallout\":"));
        let json_restored = dosimetry::ChunkRadiationData::from_json(&json_str).expect("valid json");
        assert!((json_restored.rad - original.rad).abs() < 1e-3);
        assert!((json_restored.fallout - original.fallout).abs() < 1e-3);

        // Empty data check
        let empty = dosimetry::ChunkRadiationData::default();
        assert!(empty.is_empty());
        assert_eq!(dosimetry::ChunkRadiationData::from_bytes(&[0; 4]), None);
    }

    #[test]
    fn test_world_radiation_tracker_lifecycle() {
        let mut tracker = dosimetry::WorldRadiationTracker::new();
        let dim = "minecraft:overworld";

        // Chunk load / unload
        assert!(!tracker.is_chunk_loaded(dim, 2, 3));
        tracker.on_chunk_load(dim, 2, 3);
        assert!(tracker.is_chunk_loaded(dim, 2, 3));
        tracker.on_chunk_unload(dim, 2, 3);
        assert!(!tracker.is_chunk_loaded(dim, 2, 3));

        // Add radiation & fallout
        tracker.add_radiation(dim, 0, 0, 100.0);
        tracker.add_fallout(dim, 0, 0, 50.0);
        let data = tracker.get_chunk_data(dim, 0, 0);
        assert_eq!(data.rad, 100.0);
        assert_eq!(data.fallout, 50.0);

        // Clamping to MAX
        tracker.add_radiation(dim, 0, 0, 200_000.0);
        assert_eq!(tracker.get_chunk_data(dim, 0, 0).rad, radiation::MAX_CHUNK_RADIATION);

        // Empty removal
        tracker.set_chunk_data(dim, 0, 0, dosimetry::ChunkRadiationData::default());
        assert_eq!(tracker.get_chunk_data(dim, 0, 0), dosimetry::ChunkRadiationData::default());
    }

    #[test]
    fn test_world_tracker_tick_level_diffusion_and_decay() {
        let mut tracker = dosimetry::WorldRadiationTracker::new();
        let dim = "minecraft:overworld";

        // Irradiate center chunk (0, 0)
        tracker.set_chunk_data(
            dim,
            0,
            0,
            dosimetry::ChunkRadiationData::new(100.0, 50.0),
        );

        // Run level tick (decay and diffusion)
        tracker.tick_level(dim, false);

        // Center chunk: 100.0 * 0.60 = 60.0 -> decayed: 60.0 * 0.99 - 0.05 = 59.35
        let center = tracker.get_chunk_data(dim, 0, 0);
        assert!((center.rad - 59.35).abs() < EPSILON);
        // Fallout decayed (dry): 50.0 * 0.9995 - 0.01 = 49.965
        assert!((center.fallout - 49.965).abs() < EPSILON);

        // Diffused cardinal neighbor (1, 0): 100.0 * 0.075 = 7.5 -> decayed: 7.375
        let cardinal = tracker.get_chunk_data(dim, 1, 0);
        assert!((cardinal.rad - 7.375).abs() < EPSILON);

        // Diffused diagonal neighbor (1, 1): 100.0 * 0.025 = 2.5 -> decayed: 2.425
        let diagonal = tracker.get_chunk_data(dim, 1, 1);
        assert!((diagonal.rad - 2.425).abs() < EPSILON);
    }

    #[test]
    fn test_player_ambient_exposure_calculation() {
        // Ambient chunk rad only
        let chunk_rad = 5.0;
        let empty_inv: Vec<(&str, u32)> = Vec::new();
        assert_eq!(
            dosimetry::calculate_ambient_exposure(chunk_rad, &empty_inv),
            5.0
        );

        // Chunk rad + radioactive inventory items:
        // 10 Uranium Ingots (1.5 Rad/s each) = 15.0 Rad/s
        // 2 Uranium Ore (0.5 Rad/s each) = 1.0 Rad/s
        // 64 Stone (0.0 Rad/s) = 0.0 Rad/s
        let inventory = vec![
            (content::ITEM_URANIUM_INGOT, 10),
            (content::BLOCK_URANIUM_ORE, 2),
            ("minecraft:stone", 64),
        ];
        let total_ambient = dosimetry::calculate_ambient_exposure(chunk_rad, &inventory);
        // Expected = 5.0 + (10 * 1.5) + (2 * 0.5) = 5.0 + 15.0 + 1.0 = 21.0 Rad/s
        assert!((total_ambient - 21.0).abs() < EPSILON);

        // Alternative namespace tags: "ntm:ingot_uranium"
        let alt_inventory = vec![("ntm:ingot_uranium", 4)];
        assert_eq!(
            dosimetry::calculate_ambient_exposure(0.0, &alt_inventory),
            6.0
        );
    }

    #[test]
    fn test_player_hazmat_attenuation_99_percent() {
        let ambient = 100.0_f32;

        // 0 pieces -> 100% dose rate
        let absorbed_0 = dosimetry::calculate_absorbed_exposure(ambient, 0);
        assert!((absorbed_0 - 100.0).abs() < EPSILON);

        // 2 pieces -> 90% attenuation (10% absorbed)
        let absorbed_2 = dosimetry::calculate_absorbed_exposure(ambient, 2);
        assert!((absorbed_2 - 10.0).abs() < EPSILON);

        // Full 4 pieces -> 99% attenuation (1% absorbed)
        let absorbed_4 = dosimetry::calculate_absorbed_exposure(ambient, 4);
        assert!((absorbed_4 - 1.0).abs() < EPSILON);

        // Clamped at 4 pieces max
        let absorbed_extra = dosimetry::calculate_absorbed_exposure(ambient, 8);
        assert!((absorbed_extra - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_step_player_dosimetry_accumulation() {
        let mut player = dosimetry::PlayerDosimetryData::new(0.0);
        let chunk_rad = 10.0;
        let inventory = [(content::ITEM_URANIUM_INGOT, 6)]; // 6 * 1.5 = 9.0 Rad/s
        // Total ambient = 10.0 + 9.0 = 19.0 Rad/s

        // Without hazmat: 1 tick (0.05s)
        let rate_unshielded = dosimetry::step_player_dosimetry(
            &mut player,
            chunk_rad,
            &inventory,
            0,
            0.05,
        );
        assert!((rate_unshielded - 19.0).abs() < EPSILON);
        assert_eq!(player.last_ambient_rate, 19.0);
        assert_eq!(player.last_absorbed_rate, 19.0);
        assert!((player.dose - 19.0 * 0.05).abs() < EPSILON);

        // With full hazmat suit (4 pieces): 99% attenuation -> absorbed rate = 0.19 Rad/s
        let mut protected_player = dosimetry::PlayerDosimetryData::new(0.0);
        let rate_shielded = dosimetry::step_player_dosimetry(
            &mut protected_player,
            chunk_rad,
            &inventory,
            4,
            0.05,
        );
        assert!((rate_shielded - 0.19).abs() < EPSILON);
        assert_eq!(protected_player.last_ambient_rate, 19.0);
        assert!((protected_player.last_absorbed_rate - 0.19).abs() < EPSILON);
        assert!((protected_player.dose - 0.19 * 0.05).abs() < EPSILON);

        // Over 20 ticks (1 full second)
        let mut sim_player = dosimetry::PlayerDosimetryData::new(0.0);
        for _ in 0..20 {
            dosimetry::step_player_dosimetry(&mut sim_player, 100.0, &[], 4, 0.05);
        }
        // Ambient 100 Rad/s attenuated to 1.0 Rad/s over 1 second = 1.0 Rad total dose
        assert!((sim_player.dose - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_coordinate_conversion() {
        // Block to chunk
        assert_eq!(dosimetry::block_to_chunk(0, 0), (0, 0));
        assert_eq!(dosimetry::block_to_chunk(15, 15), (0, 0));
        assert_eq!(dosimetry::block_to_chunk(16, 32), (1, 2));
        assert_eq!(dosimetry::block_to_chunk(-1, -1), (-1, -1));
        assert_eq!(dosimetry::block_to_chunk(-16, -17), (-1, -2));

        // Float position to chunk
        assert_eq!(dosimetry::pos_to_chunk(15.9, -0.1), (0, -1));
        assert_eq!(dosimetry::pos_to_chunk(35.5, 48.2), (2, 3));
    }

    #[test]
    fn test_runtime_state_integration() {
        reset_state();
        let dim = "minecraft:overworld";
        let uuid = 42u128;

        // Chunks
        on_chunk_load(dim, [1, 2]);
        set_chunk_radiation(dim, 1, 2, 50.0);
        assert_eq!(get_chunk_radiation(dim, 1, 2), 50.0);

        // Level tick
        on_level_tick(dim);
        let center = get_chunk_radiation(dim, 1, 2);
        // 50.0 * 0.60 = 30.0 -> 30.0 * 0.99 - 0.05 = 29.65
        assert!((center - 29.65).abs() < EPSILON);

        // Player exposure record
        let inv = [(content::ITEM_URANIUM_INGOT, 2)];
        let rate = record_player_exposure(uuid, center, &inv, 4, 0.05);
        // Ambient = 29.65 + 3.0 = 32.65 -> full hazmat 1% = 0.3265
        assert!((rate - 0.3265).abs() < EPSILON);

        let data = get_player_dosimetry(uuid);
        assert!((data.dose - 0.3265 * 0.05).abs() < EPSILON);
        assert!((data.last_ambient_rate - 32.65).abs() < EPSILON);
        assert!((data.last_absorbed_rate - 0.3265).abs() < EPSILON);

        set_player_dose(uuid, 500.0);
        assert_eq!(get_player_dosimetry(uuid).dose, 500.0);

        on_chunk_unload(dim, [1, 2]);
        reset_state();
    }

    #[test]
    fn test_radiation_sickness_stages_and_thresholds() {
        // Healthy: dose < 200 Rad
        let healthy_eval = sickness::evaluate_sickness(0.0, 100);
        assert_eq!(healthy_eval.stage, sickness::RadiationSicknessStage::None);
        assert!(healthy_eval.effects.is_empty());
        assert_eq!(healthy_eval.damage, None);

        let sub_mild_eval = sickness::evaluate_sickness(199.9, 100);
        assert_eq!(sub_mild_eval.stage, sickness::RadiationSicknessStage::None);
        assert!(sub_mild_eval.effects.is_empty());

        // Mild: 200 <= dose < 500 Rad -> Nausea and Slowness
        let mild_eval = sickness::evaluate_sickness(200.0, 100);
        assert_eq!(mild_eval.stage, sickness::RadiationSicknessStage::Mild);
        assert_eq!(mild_eval.damage, None);
        let effect_ids: Vec<&str> = mild_eval.effects.iter().map(|e| e.id).collect();
        assert!(effect_ids.contains(&sickness::EFFECT_NAUSEA));
        assert!(effect_ids.contains(&sickness::EFFECT_SLOWNESS));
        assert!(!effect_ids.contains(&sickness::EFFECT_WEAKNESS));

        let sub_severe_eval = sickness::evaluate_sickness(499.9, 100);
        assert_eq!(sub_severe_eval.stage, sickness::RadiationSicknessStage::Mild);

        // Severe: 500 <= dose < 1000 Rad -> Nausea, Slowness, Weakness, Mining Fatigue, periodic damage
        let severe_interval = sickness::evaluate_sickness(500.0, 100);
        assert_eq!(severe_interval.stage, sickness::RadiationSicknessStage::Severe);
        let severe_effect_ids: Vec<&str> = severe_interval.effects.iter().map(|e| e.id).collect();
        assert!(severe_effect_ids.contains(&sickness::EFFECT_NAUSEA));
        assert!(severe_effect_ids.contains(&sickness::EFFECT_SLOWNESS));
        assert!(severe_effect_ids.contains(&sickness::EFFECT_WEAKNESS));
        assert!(severe_effect_ids.contains(&sickness::EFFECT_MINING_FATIGUE));
        assert_eq!(severe_interval.damage, Some(sickness::SEVERE_PERIODIC_DAMAGE));
        assert_eq!(severe_interval.damage_type, Some(sickness::DAMAGE_TYPE_RADIATION));

        // Severe on non-periodic tick -> no damage
        let severe_non_interval = sickness::evaluate_sickness(500.0, 101);
        assert_eq!(severe_non_interval.stage, sickness::RadiationSicknessStage::Severe);
        assert_eq!(severe_non_interval.damage, None);

        let sub_lethal = sickness::evaluate_sickness(999.9, 100);
        assert_eq!(sub_lethal.stage, sickness::RadiationSicknessStage::Severe);

        // Lethal: dose >= 1000 Rad -> Lethal organ damage via custom ntm:radiation
        let lethal_eval = sickness::evaluate_sickness(1000.0, 101);
        assert_eq!(lethal_eval.stage, sickness::RadiationSicknessStage::Lethal);
        assert_eq!(lethal_eval.damage, Some(sickness::LETHAL_DAMAGE_AMOUNT));
        assert_eq!(lethal_eval.damage_type, Some(sickness::DAMAGE_TYPE_RADIATION));
        assert!(sickness::is_radiation_damage(sickness::DAMAGE_TYPE_RADIATION));

        let lethal_extreme = sickness::evaluate_sickness(2500.0, 50);
        assert_eq!(lethal_extreme.stage, sickness::RadiationSicknessStage::Lethal);
        assert_eq!(lethal_extreme.damage, Some(sickness::LETHAL_DAMAGE_AMOUNT));
    }

    #[test]
    fn test_natural_dose_recovery_in_clean_environment_vs_exposed() {
        // In clean environment (ambient = 0.0), natural recovery rate is 0.05 Rad/s
        let mut recovering_player = dosimetry::PlayerDosimetryData::new(100.0);
        for _ in 0..20 {
            // 20 ticks * 0.05s = 1.0 second
            dosimetry::step_player_dosimetry(&mut recovering_player, 0.0, &[], 0, 0.05);
        }
        // Expected reduction: 0.05 Rad -> 99.95 Rad
        assert!((recovering_player.dose - 99.95).abs() < EPSILON);

        // Test clamping at 0.0
        let mut low_dose_player = dosimetry::PlayerDosimetryData::new(0.02);
        dosimetry::step_player_dosimetry(&mut low_dose_player, 0.0, &[], 0, 1.0);
        assert_eq!(low_dose_player.dose, 0.0);

        // In contaminated environment (ambient > 0), no recovery occurs; dose increases
        let mut exposed_player = dosimetry::PlayerDosimetryData::new(100.0);
        dosimetry::step_player_dosimetry(&mut exposed_player, 10.0, &[], 0, 1.0);
        // Ambient 10.0 Rad/s for 1.0s -> dose = 100.0 + 10.0 = 110.0 Rad
        assert!((exposed_player.dose - 110.0).abs() < EPSILON);
    }

    #[test]
    fn test_player_dosimetry_binary_serialization_roundtrip() {
        let original = dosimetry::PlayerDosimetryData {
            dose: 345.67,
            last_ambient_rate: 12.5,
            last_absorbed_rate: 0.125,
            ticks_alive: 4242,
        };

        let bytes = original.to_bytes();
        assert_eq!(bytes.len(), 20);

        let deserialized = dosimetry::PlayerDosimetryData::from_bytes(&bytes).unwrap();
        assert!((deserialized.dose - original.dose).abs() < EPSILON);
        assert!((deserialized.last_ambient_rate - original.last_ambient_rate).abs() < EPSILON);
        assert!((deserialized.last_absorbed_rate - original.last_absorbed_rate).abs() < EPSILON);
        assert_eq!(deserialized.ticks_alive, original.ticks_alive);

        // Backwards compatibility with 12-byte slices (without ticks_alive)
        let slice_12 = &bytes[0..12];
        let compat = dosimetry::PlayerDosimetryData::from_bytes(slice_12).unwrap();
        assert!((compat.dose - original.dose).abs() < EPSILON);
        assert_eq!(compat.ticks_alive, 0);

        // Too short slice (< 12 bytes) fails safely
        assert!(dosimetry::PlayerDosimetryData::from_bytes(&[0u8; 8]).is_none());
    }

    #[test]
    fn test_player_dosimetry_json_serialization_roundtrip() {
        let original = dosimetry::PlayerDosimetryData {
            dose: 789.1234,
            last_ambient_rate: 55.4,
            last_absorbed_rate: 0.554,
            ticks_alive: 8888,
        };

        let json = original.to_json();
        assert!(json.contains("\"dose\":789.1234"));
        assert!(json.contains("\"ticks\":8888"));

        let parsed = dosimetry::PlayerDosimetryData::from_json(&json).unwrap();
        assert!((parsed.dose - original.dose).abs() < EPSILON);
        assert!((parsed.last_ambient_rate - original.last_ambient_rate).abs() < EPSILON);
        assert!((parsed.last_absorbed_rate - original.last_absorbed_rate).abs() < EPSILON);
        assert_eq!(parsed.ticks_alive, original.ticks_alive);

        // Invalid JSON fails safely
        assert!(dosimetry::PlayerDosimetryData::from_json("invalid json").is_none());
        assert!(dosimetry::PlayerDosimetryData::from_json(r#"{"dose": "abc"}"#).is_none());
    }

    #[test]
    fn test_player_sickness_tick_state_integration() {
        reset_state();
        let uuid = 9999u128;

        // Start healthy
        set_player_dose(uuid, 0.0);
        let eval_healthy = tick_player_sickness(uuid);
        assert_eq!(eval_healthy.stage, sickness::RadiationSicknessStage::None);

        // Escalate to Mild
        set_player_dose(uuid, 250.0);
        let eval_mild = tick_player_sickness(uuid);
        assert_eq!(eval_mild.stage, sickness::RadiationSicknessStage::Mild);
        assert_eq!(eval_mild.damage, None);

        // Escalate to Severe
        set_player_dose(uuid, 600.0);
        let eval_severe = tick_player_sickness(uuid);
        assert_eq!(eval_severe.stage, sickness::RadiationSicknessStage::Severe);

        // Escalate to Lethal
        set_player_dose(uuid, 1200.0);
        let eval_lethal = tick_player_sickness(uuid);
        assert_eq!(eval_lethal.stage, sickness::RadiationSicknessStage::Lethal);
        assert_eq!(eval_lethal.damage, Some(sickness::LETHAL_DAMAGE_AMOUNT));
        assert_eq!(eval_lethal.damage_type, Some(sickness::DAMAGE_TYPE_RADIATION));
        // Lethal execution resets dose
        assert_eq!(get_player_dosimetry(uuid).dose, 0.0);

        // Save & load session persistence
        set_player_dose(uuid, 350.0);
        let bytes = save_player_dosimetry(uuid);
        let json = save_player_dosimetry_json(uuid);

        reset_state();
        assert_eq!(get_player_dosimetry(uuid).dose, 0.0);

        // Restore from binary
        assert!(load_player_dosimetry(uuid, &bytes));
        assert!((get_player_dosimetry(uuid).dose - 350.0).abs() < EPSILON);

        // Restore from JSON
        reset_state();
        assert!(load_player_dosimetry_json(uuid, &json));
        assert!((get_player_dosimetry(uuid).dose - 350.0).abs() < EPSILON);

        // Test on_player_death resets dose
        on_player_death(uuid);
        assert_eq!(get_player_dosimetry(uuid).dose, 0.0);

        reset_state();
    }
}
