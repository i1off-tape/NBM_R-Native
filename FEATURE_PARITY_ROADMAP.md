# NBM_R-Native: Complete Feature Parity Roadmap
## Exhaustive Java HBM (`com.hbm.*`) to Horizon Native Porting Matrix

This document provides a complete registry of all subsystems, packages, and mechanics from HBM's Nuclear Tech Mod (`hbm-s-nuclear-tech-git`). It defines the exact phase breakdown, target architecture, and verification criteria to guarantee 100% functional parity in Horizon Native Rust.

---

## 1. Three-Tier Porting Architecture

To prevent architectural drift and ensure that playable, verified physics loops exist early, all features are organized into three sequential tiers:

- **Tier 1 (Core Physics, Hazards, Energy and Primary Industry)**: The fundamental simulation foundation. Includes ionizing radiation, chunk contamination, HE cable networks, primary metallurgy/enrichment machines, the RBMK modular nuclear reactor, and nuclear blast raymarching.
- **Tier 2 (Advanced Chemistry, Heavy Industry, Missiles, Logistics)**: The military and advanced industrial systems. Includes multi-tank chemical plants, fractional distillation, missile silos, radar guidance, CIWS/turrets, item/fluid logistics, and DBM reactors.
- **Tier 3 (Worldgen, Structures, Weaponry, Mobs, Audio and Auxiliary Content)**: Exploration and tactical gear. Includes ore vein placement, meteor strikes, radioactive dungeons, hand-held firearms, armor systems, custom particle effects, and ambient audio.

---

## 2. Complete Java HBM Subsystem Registry

### Subsystem 1: Radiation, Toxicity and Environmental Hazards
- **Java Source Packages**: `com.hbm.hazard.*`, `com.hbm.saveddata.RadiationSavedData`, `com.hbm.extprop.HbmLivingProps`, `com.hbm.handler.radiation.*`, `com.hbm.potion.*`
- **Tier**: Tier 1
- **Status**: Core Parity Achieved (Spec 7: Core Physics, Dosimetry, Hazmat, Sickness, Geiger)
- **Target Location**: `mixins/nbm_r/runtime.rs`, `mixins/nbm_r/hud.rs`, `mixins/nbm_r/tick.rs`, `data/nbm_r/horizon/`
- **Key Mechanics**:
  - `[x]` Chunk radiation tracking: Runtime `WorldRadiationTracker` with persistence & level ticks.
  - `[x]` Environmental decay and 8-way mass-conserving gradient diffusion in `radiation.rs`.
  - `[ ]` Rain fallout washout transferring atmospheric fallout into surface block contamination (Deferred: surface block infection).
  - `[x]` Entity dose accumulation: chunk exposure plus inventory radioactive items emission.
  - `[x]` Protective gear attenuation: Hazmat armor 99% attenuation for full suit (Lead armor deferred).
  - `[x]` Biological symptom thresholds: 5 sickness stages & lethal death at 1000 Rad with single death command and effect cleanup.
  - `[x]` Interactive Geiger counter audio clicks ($\log_{10}(\text{rads})$ frequency) and corner HUD dial in `native_hud`.
  - `[ ]` Secondary hazards: Asbestos inhalation, coal dust explosion hazard, cryogenic freezing, and flash blindness (Deferred to Tier 3).

### Subsystem 2: Energy Networks (HE - Hbm Energy)
- **Java Source Packages**: `api.hbm.energymk2.*`, `com.hbm.tileentity.conductor.*`, `com.hbm.blocks.network.*`
- **Tier**: Tier 1
- **Status**: Architectural Specification Recorded (ADR-0001)
- **Target Location**: `mixins/nbm_r/runtime/energy.rs`, `mixins/nbm_r/level_tick.rs`
- **Key Mechanics**:
  - `[ ]` Machine capability `ModEnergyBuffer` with storage, capacity, max input, and max output.
  - `[ ]` Cached component energy networks using disjoint-set tracking updated on block place/break events.
  - `[ ]` Generator-to-consumer distribution in `level_tick_pre` operating without per-tick pathfinding.
  - `[ ]` Wire burning, overvoltage fuses, and transformer tier stepped conversion (LV, MV, HV, EHV).

### Subsystem 3: Fluid Dynamics and Chemical Processing
- **Java Source Packages**: `com.hbm.inventory.fluid.*`, `com.hbm.inventory.recipes.*`, `com.hbm.blocks.fluid.*`, `com.hbm.tileentity.machine.TileEntityMachineChemplant`
- **Tier**: Tier 1 and Tier 2
- **Status**: Not Started
- **Target Location**: `data/ntm/horizon/fluid/*.json`, `mixins/nbm_r/runtime/fluid.rs`, `mixins/nbm_r/runtime/machine/chemplant.rs`
- **Key Mechanics**:
  - `[ ]` Native fluid definitions: Steam, Superheated Steam, Heavy Water, Petroleum, Sulfuric Acid, UF6, Deuterium, Tritium.
  - `[ ]` Pipe networks: pressurized gas and liquid transport with cached flow rates.
  - `[ ]` Machine multi-tank capacity (`ModFluidTanks` supporting multiple input and output fluid types).
  - `[ ]` Chemical synthesis: cracking towers, hydrotreaters, catalytic reformers, and centrifuge isotope separation.

### Subsystem 4: Primary Machinery and Metallurgy
- **Java Source Packages**: `com.hbm.tileentity.machine.*`, `com.hbm.inventory.container.*`, `com.hbm.inventory.gui.*`, `com.hbm.crafting.*`
- **Tier**: Tier 1
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/machine/*.rs`, `data/ntm/horizon/menu/*.json`
- **Key Mechanics**:
  - `[ ]` Machine base wrapper over `ModBlockEntity` handling power, 16-bit network sync data, and item slots.
  - `[ ]` Isotope Centrifuge: enrichment of Uranium-235, Plutonium-239, and Thorium-232.
  - `[ ]` Industrial Blast Furnace: high-temperature smelting of steel, titanium, and tungsten alloys.
  - `[ ]` Shredder and Crusher: ore multiplication, dust processing, and scrap recycling.
  - `[ ]` Assembly Machine and Stamp Mill: progressive component manufacturing and casing shaping.

### Subsystem 5: RBMK Nuclear Reactor and Core Physics
- **Java Source Packages**: `com.hbm.tileentity.machine.rbmk.*`, `com.hbm.handler.neutron.*`
- **Tier**: Tier 1
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/rbmk/*.rs`, `mixins/nbm_r/machine_tick.rs`
- **Key Mechanics**:
  - `[ ]` Modular column lattice simulation: fuel columns, control rods, moderator columns, boiler columns, reflector walls.
  - `[ ]` Neutron kinetics: fast neutron emission, moderation to thermal neutrons, delayed neutron precursor decay.
  - `[ ]` Xenon-135 poisoning and reactivity transients.
  - `[ ]` Control rod absorption depth modulation ($0\%$ to $100\%$) and automatic SCRAM triggers.
  - `[ ]` Thermal-hydraulic boiling: water consumption, saturated and superheated steam generation.
  - `[ ]` Supercritical core meltdown: structural failure, Corium generation, and positive void coefficient explosion.
  - `[ ]` Central RBMK Console: remote column telemetry, graphical monitoring display, automated rod choreography.

### Subsystem 6: High-Performance Blast and Nuclear Physics Engine
- **Java Source Packages**: `com.hbm.explosion.*`, `com.hbm.entity.logic.EntityNukeExplosionMK3`
- **Tier**: Tier 1
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/explosion.rs`, `mixins/nbm_r/render/warp.rs`
- **Key Mechanics**:
  - `[ ]` Tick-budgeted raymarched spherical blast waves (bounded block destruction quota per tick in `level_tick_post`).
  - `[ ]` Client mesh freeze (`native_world_hold`) to prevent frame hitching during blast epicenters.
  - `[ ]` Custom GPU shockwave refraction shader injected into wgpu pipeline via `native_world_warp`.
  - `[ ]` Thermal flash vaporization, firestorms, and deep crater generation with smooth falloff.
  - `[ ]` Severe fallout plume deposition scaling with blast energy yield.

### Subsystem 7: Missiles, Ballistics, Radars and Defense
- **Java Source Packages**: `com.hbm.entity.missile.*`, `com.hbm.tileentity.bomb.*`, `com.hbm.tileentity.turret.*`
- **Tier**: Tier 2
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/entity/missile.rs`, `mixins/nbm_r/runtime/turret.rs`
- **Key Mechanics**:
  - `[ ]` Launch pad and missile silo automation: target coordinate alignment and propellant fueling.
  - `[ ]` Ballistic missile trajectories calculated over world tick intervals with chunk pre-loading.
  - `[ ]` Phased array early warning radar: detects incoming missiles within $4096$ block spherical radius.
  - `[ ]` Automated CIWS defense turrets: targeting algorithms intercepting enemy warheads in flight.

### Subsystem 8: Logistics, Transport and Automation
- **Java Source Packages**: `com.hbm.tileentity.network.*`, `com.hbm.uninos.*`, `com.hbm.wiaj.*`, `com.hbm.entity.train.*`, `com.hbm.entity.cart.*`
- **Tier**: Tier 2
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/logistics/*.rs`
- **Key Mechanics**:
  - `[ ]` Overhead crane gantry systems and freight container loading.
  - `[ ]` High-speed industrial conveyors, item splitters, and smart sorters.
  - `[ ]` Industrial narrow-gauge rail transport, tank wagons, and automated flatbed freight cars.

### Subsystem 9: Advanced Energy Generation (Fusion, Tokamak, DBM, RTG)
- **Java Source Packages**: `com.hbm.tileentity.machine.fusion.*`, `com.hbm.tileentity.machine.TileEntityCore*`, `com.hbm.tileentity.machine.TileEntityMachineZirnox`
- **Tier**: Tier 2
- **Status**: Not Started
- **Target Location**: `mixins/nbm_r/runtime/reactor/*.rs`
- **Key Mechanics**:
  - `[ ]` DBM (Dark Boiler Machine) matter annihilation and antimatter containment.
  - `[ ]` Magnetic confinement Tokamak / ITER fusion reactors with plasma stability management.
  - `[ ]` ZIRNOX natural uranium zirconium-clad heavy water power reactors.
  - `[ ]` Passive radioisotope thermoelectric generators (RTG) with isotope half-life output decay.

### Subsystem 10: World Generation, Structures, Ores and Exploration
- **Java Source Packages**: `com.hbm.world.*`, `com.hbm.world.gen.*`, `com.hbm.world.dungeon.*`
- **Tier**: Tier 3
- **Status**: Not Started
- **Target Location**: `data/ntm/horizon/worldgen/`, `mixins/nbm_r/runtime/worldgen.rs`
- **Key Mechanics**:
  - `[ ]` Deep underground ore veins: Uranium, Thorium, Titanium, Tungsten, Lead, Beryllium, Fluorite, Lithium.
  - `[ ]` Surface features: oil geysers, petroleum tar lakes, acid ponds.
  - `[ ]` Dangerous procedural structures: abandoned missile silos, radar stations, rad-waste test vaults, bunker ruins.
  - `[ ]` Dynamic world events: atmospheric meteor strikes with impact craters and rare extraterrestrial minerals.

### Subsystem 11: Firearms, Heavy Weapons, Armor and Survival
- **Java Source Packages**: `com.hbm.items.weapon.*`, `com.hbm.items.armor.*`, `com.hbm.items.tool.*`, `com.hbm.handler.guncfg.*`
- **Tier**: Tier 3
- **Status**: Not Started
- **Target Location**: `data/ntm/horizon/item/`, `mixins/nbm_r/runtime/combat.rs`, `mixins/nbm_r/key.rs`
- **Key Mechanics**:
  - `[ ]` Lead-lined, Hazmat, HEV, and Power Armor suits with modular energy/oxygen life support.
  - `[ ]` Ballistic projectile weapons: recoil, reload mechanics, spread, and specialized ammunition (AP, HE, Depleted Uranium).
  - `[ ]` Hand-held radiation dosimeters, survey meters, and night-vision goggle HUD overlays.

### Subsystem 12: Audio, Particles, Rendering and Visual FX
- **Java Source Packages**: `com.hbm.render.*`, `com.hbm.particle.*`, `com.hbm.sound.*`, `com.hbm.animloader.*`
- **Tier**: Tier 3
- **Status**: Not Started
- **Target Location**: `assets/ntm/`, `mixins/nbm_r/render/*.rs`
- **Key Mechanics**:
  - `[ ]` Volumetric mushroom clouds, shockwave rings, and ionization glow rendered via `wgpu`.
  - `[ ]` Dynamic Geiger counter audio synthesis with volume and pitch variations.
  - `[ ]` Siren systems: customizable mechanical and electronic civil defense alarms with distance attenuation.
  - `[ ]` Modern block entity geometry submission using Horizon's `block_entity_submit` render queue.

---

## 3. Mathematical Parity and Verification Protocol

All simulation algorithms must adhere to the following verification rules:

1. **Pure Rust Decoupling**:
   Simulation algorithms must reside in pure Rust functions within `runtime.rs` or its submodules, taking only primitive data types or mod-local structs without coupling to Horizon engine types.
2. **Formula Unit Tests**:
   For every ported algorithm, a unit test must be written under `#[cfg(test)]` verifying identical mathematical output against reference values from original Java classes:
   $$\left| \text{result}_{\text{Rust}} - \text{result}_{\text{Java}} \right| < 10^{-4}$$
3. **Automated CI Validation**:
   Every code change must pass:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --locked --offline -- -D warnings
   cargo test --workspace --locked --offline
   python3 tools/test_template.py
   python3 tools/package.py
   ```
