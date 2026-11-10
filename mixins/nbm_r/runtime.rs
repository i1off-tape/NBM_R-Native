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
}
