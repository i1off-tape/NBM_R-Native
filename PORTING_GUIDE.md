# NBM_R-Native: Comprehensive Porting Guide
## Porting Hbm's Nuclear Tech Mod (Java) to Horizon Native (Rust)

This document provides an exhaustive, step-by-step engineering specification for porting **Hbm's Nuclear Tech Mod (NTM)** from Minecraft 1.7.10/Forge (`com.hbm.*`) to the **Horizon** native voxel engine (`.hnmod` on Rust).

---

## 1. Architectural Philosophy

### Java NTM vs. Horizon Native
| Aspect | Original NTM (Java / Forge 1.7.10) | NBM_R-Native (Rust / Horizon) |
|---|---|---|
| **Runtime Model** | JVM, garbage collected, Forge EventBus | AOT compiled, zero GC, static mixin hooks |
| **Modification Layer** | Bytecode manipulation, ASM, coremods | `horizon-mixins` AST patching + Trait API |
| **Blocks & Items** | Java classes extending `Block` / `Item` | Content JSON templates (`data/ntm/horizon/...`) |
| **Tile Entities** | Java classes extending `TileEntity` | `ModBlockEntity` with `nbt`, `items`, `data` |
| **Data Storage** | `WorldSavedData` (`.dat` files) | Horizon Server Attachments (`Holder::Level/Chunk`) |
| **Networking** | Custom Forge SimpleNetworkWrapper (`IPacket`) | Dedicated `horizon:mod_net` channels + raw packets |
| **Rendering** | OpenGL 1.2 / 2.1 fixed pipeline, Tessellator | Modern `wgpu` (Vulkan/Metal), WGSL shaders, `HudBatch` |

---

## 2. Subsystem Technical Blueprint

### System 1: Content Foundation (Declarative `.hmod`)
Before writing complex Rust logic, all static definitions must be declared in Horizon's data format:

1. **Items & Materials**:
   - Ingots (Uranium-235, Plutonium-239, Titanium, Tungsten, Lead, Beryllium).
   - Fuel rods, plates, wires, radioactive isotopes.
   - Files: `data/ntm/horizon/item/<item_id>.json` copying `minecraft:iron_ingot` or `minecraft:stick` templates, with properties (`max_stack_size`, `rarity`).
2. **Blocks & Ores**:
   - Uranium Ore, Thorium Ore, Lead Ore, Basalt, Concrete, Lead Bricks.
   - Files: `data/ntm/horizon/block/<block_id>.json` copying `minecraft:iron_block` or `minecraft:stone`, setting `hardness`, `sounds`, `occludes`.
3. **Fluids**:
   - Steam, Superheated Steam, Heavy Water, Petroleum, Sulfuric Acid, UF6 (Uranium Hexafluoride).
   - Files: `data/ntm/horizon/fluid/<fluid_id>.json` copying `minecraft:water` or `minecraft:lava`, specifying `viscosity`, `density`, `tick_rate`, textures.
4. **Creative Tabs**:
   - Parts, Nuclear, Machines, Missiles, Weapons.
   - Files: `data/ntm/horizon/creative_tab/<tab_name>.json`.

---

### System 2: Radiation, Hazards & Environmental Contamination

#### Original Reference:
- `com.hbm.hazard.HazardSystem`
- `com.hbm.saveddata.RadiationSavedData`
- `com.hbm.extprop.HbmLivingProps`

#### Horizon Rust Design:
1. **Chunk Radiation Storage**:
   Use **Horizon Attachments** on `Holder::Chunk([cx, cz])`:
   ```rust
   #[derive(Default, Serialize, Deserialize)]
   pub struct ChunkRadiation {
       pub rads: f32,       // Background gamma/neutron rads
       pub fallout: f32,    // Physical isotope particulate contamination
   }
   ```
2. **Environmental Propagation (World Tick)**:
   In hook `level_tick_post`:
   - Iterate over active radioactive chunks.
   - Decay radiation over time: `rads *= 0.999f32;`.
   - Diffuse radiation to adjacent 4 chunks based on gradient.
   - Rain washes `fallout` into ground/water tables.
3. **Player & Living Entity Exposure**:
   In hook `player_tick_post` / `entity_tick_post`:
   - Dose accumulated = `chunk_rads * distance_factor + inventory_rads`.
   - Attenuation: Check armor slots for Lead/Hazmat tags (tag `#ntm:hazmat` reduces dose by 99%).
   - Thresholds:
     - 200 Rads: Nausea, Slowness.
     - 500 Rads: Weakness, Mining Fatigue, periodic damage.
     - 1000 Rads: Lethal organ failure (`living_incoming_damage_v2` with custom damage type `ntm:radiation`).
4. **Geiger Counter & Dosage HUD**:
   In hook `native_hud`:
   - Read player's current dose.
   - Play audio click (`native_play_sound`) with frequency proportional to $\log_{10}(\text{rads})$.
   - Draw dial gauge in screen corner via `corners.place(...)`.

---

### System 3: Energy (HE - Hbm Energy) & Cable Networks

#### Original Reference:
- `api.hbm.energymk2.IEnergyConnectorMK2`
- `com.hbm.tileentity.conductor.*`

#### Horizon Rust Design:
1. **Machine Capability**:
   Attach `ModEnergyBuffer` capability to machine `ModBlockEntity`:
   ```rust
   pub struct EnergyStorage {
       pub stored: i64,
       pub capacity: i64,
       pub max_input: i64,
       pub max_output: i64,
   }
   ```
2. **Cable Networks (Breadth-First Search / Union-Find)**:
   - When a cable block is placed (`place_block`), merge adjacent cable nodes into a network graph.
   - In `level_tick_pre`, solve energy distribution:
     - Sum total generation from all connected generators.
     - Distribute proportionally to consumers requesting power.
     - Apply wire burning / overvoltage if voltage exceeds cable rating!

---

### System 4: Machine Architecture & ModBlockEntity

#### Original Reference:
- `com.hbm.tileentity.machine.TileEntityMachineCentrifuge`
- `com.hbm.tileentity.machine.TileEntityMachineBlastFurnace`

#### Horizon Rust Design:
1. **Block Entity Lifecycle**:
   - Register via `stateful_block_entity`:
     - `id: "ntm:centrifuge"`
     - `items`: 4 slots (1 input, 3 outputs)
     - `data`: `[progress, max_progress, power, max_power]` (16-bit network sync)
     - `nbt`: persistent machine settings
2. **Tick Simulation (`block_entity_tick`)**:
   ```rust
   pub fn tick_centrifuge(entity: &mut ModBlockEntity, host: &mut Host) {
       let power = &mut entity.data[2];
       let progress = &mut entity.data[0];
       
       if *power >= CENTRIFUGE_DRAIN {
           *power -= CENTRIFUGE_DRAIN;
           *progress += 1;
           if *progress >= CENTRIFUGE_DURATION {
               *progress = 0;
               process_centrifuge_recipe(&mut entity.items);
           }
       }
   }
   ```
3. **Container UI (`native_screen`)**:
   - Menu layout defined in `data/ntm/horizon/menu/centrifuge.json`.
   - `screen_draw` renders custom NTM gauge textures.

---

### System 5: RBMK Nuclear Reactor & Physics

#### Original Reference:
- `com.hbm.tileentity.machine.rbmk.*`
- `com.hbm.tileentity.machine.rbmk.TileEntityRBMKConsole`

#### Horizon Rust Design:
The RBMK is a multi-column modular reactor composed of:
1. **Fuel Columns (`TileEntityRBMKRod`)**:
   - State: Fuel type (Uranium, MOX, Plutonium), Burnup %, Core Temperature, Flux output.
   - Emits fast/thermal neutrons into adjacent 4 columns.
2. **Control Rods (`TileEntityRBMKControl`)**:
   - Insertion level (0% to 100%).
   - Absorbs neutrons proportionally to insertion depth.
3. **Moderator Columns (Graphite, Beryllium)**:
   - Moderates fast neutrons into thermal neutrons (which increase fission cross-section).
4. **Boiler / Heat Exchanger Columns**:
   - Consumes Water, produces Steam depending on temperature.
5. **Meltdown & Supercritical Explosion**:
   - If column temperature exceeds 3000°C:
     - Rods melt down into Corium.
     - Positive void coefficient triggers prompt criticality.
     - Calls `ExplosionEngine::detonate_rbmk(pos)` destroying the reactor building and creating massive fallout.

---

### System 6: Explosions, Ballistics & Nuclear Physics

#### Original Reference:
- `com.hbm.explosion.ExplosionNukeGeneric`
- `com.hbm.explosion.ExplosionChaos`

#### Horizon Rust Design:
Nuclear explosions in Java Minecraft cause devastating server lag. In Horizon Rust, we achieve 60 FPS nuclear explosions using:
1. **Chunk Batching & Rate Limiting**:
   - Do NOT break 1,000,000 blocks in a single tick.
   - Queue blast wave rays; process a fixed budget (e.g. 5,000 blocks/tick) in `level_tick_post`.
2. **World Hold (`native_world_hold`)**:
   - Freeze the client chunk mesh rebuild during the initial blast frame to eliminate stutters.
3. **GPU Shockwave Shader (`native_world_warp`)**:
   - Inject a custom WGSL shader into Horizon's wgpu pipeline:
   - Distorts vertices radially from the blast epicenter creating a realistic atmospheric shockwave refraction.
4. **Mushroom Cloud Particle Geometry**:
   - Render volumetric torus/cylinder via `RenderHooks::world_drawn`.

---

## 3. Implementation Milestones

```
Milestone 1: Project Setup & Test Harness (Current)
   └── Scaffold, AGENTS.md, PORTING_GUIDE.md, MAPPING.md, cargo test passing.

Milestone 2: Data Foundations & Content (Weeks 1-2)
   └── Ore blocks, ingots, fluids, creative tabs, recipes.

Milestone 3: Hazards & Radiation Engine (Weeks 3-4)
   └── Chunk radiation attachments, exposure calculations, Geiger HUD.

Milestone 4: Core Machinery (Weeks 5-6)
   └── Centrifuge, Blast Furnace, Chemical Plant, HE energy system.

Milestone 5: Advanced Nuclear Tech (Weeks 7-9)
   └── RBMK Reactor columns, steam loops, turbine generators.

Milestone 6: Nuclear Weapons & Explosions (Weeks 10-12)
   └── Missiles, nuclear blast engine, fallout contamination, wgpu shaders.
```
