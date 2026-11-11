# Java HBM to Horizon Native (Rust) Mapping Matrix

This document maps Java packages, classes, and mechanics from **Hbm's Nuclear Tech Mod** (`hbm-s-nuclear-tech-git`) to their target locations in **NBM_R-Native** and the Horizon Engine.

---

## 1. Package-Level Mapping

| Java Package (`com.hbm.*`) | Destination in Horizon Native | Technology / API Used |
|---|---|---|
| `com.hbm.blocks.*` | `data/ntm/horizon/block/*.json` | Declarative templates (`template: "minecraft:..."`) |
| `com.hbm.items.*` | `data/ntm/horizon/item/*.json` | Declarative templates (`components`, `rarity`) |
| `com.hbm.creativetabs.*` | `data/ntm/horizon/creative_tab/*.json` | Horizon creative tab declarations |
| `com.hbm.inventory.fluid.*` | `data/ntm/horizon/fluid/*.json` | Fluid declarations + `ModFluidTanks` in Rust |
| `com.hbm.hazard.*` | `mixins/nbm_r/runtime/hazard.rs` | Horizon Attachments + `player_tick_post` |
| `com.hbm.saveddata.*` | `mixins/nbm_r/runtime/saveddata.rs` | `Holder::Chunk` / `Holder::Level` Attachments |
| `com.hbm.tileentity.machine.*` | `mixins/nbm_r/runtime/machine/*.rs` | `ModBlockEntity` + `block_entity_tick` |
| `com.hbm.tileentity.machine.rbmk.*` | `mixins/nbm_r/runtime/rbmk/*.rs` | Modular `ModBlockEntity` network simulation |
| `com.hbm.inventory.gui.*` | `data/ntm/horizon/menu/*.json` | Horizon Containers (`native_screen`) |
| `com.hbm.explosion.*` | `mixins/nbm_r/runtime/explosion/*.rs` | Horizon World Hooks (`native_world_hold`) |
| `com.hbm.entity.missile.*` | `mixins/nbm_r/runtime/entity/*.rs` | `entity_tick_post`, projectile prediction |
| `com.hbm.packet.*` | `mixins/nbm_r/runtime/net.rs` | Channel `horizon:mod_net` |
| `com.hbm.render.*` | `mixins/nbm_r/render/*.rs` | `block_entity_submit`, `RenderHooks` (wgpu) |

---

## 2. Key Class Mapping Reference

### Hazards & Radiation
| Original Java Class | Target Rust Implementation | Notes |
|---|---|---|
| `com.hbm.hazard.HazardSystem` | `nbm_r::hazard::HazardSystem` | Pure functions calculating biological effect from Rads/sec. |
| `com.hbm.hazard.HazardRegistry` | `nbm_r::content` | Radiation emission values: `HazardRegistry.u` = 0.35 Rad/s. |
| `com.hbm.handler.radiation.ChunkRadiationManager` | `nbm_r::dosimetry::WorldRadiationTracker` | 20-tick cadence update (`eggTimer >= 20`). |
| `com.hbm.handler.radiation.ChunkRadiationHandlerNT` | `nbm_r::radiation` | Chunk radiation decay: `p.radiation *= 0.999F; p.radiation -= 0.05F`. |
| `com.hbm.handler.EntityEffectHandler` | `nbm_r::sickness` & `mixins/nbm_r/tick.rs` | 5 sickness tiers (200, 400, 600, 800, 1000 Rad), Poison, Wither, instant death. |
| `com.hbm.extprop.HbmLivingProps` | `nbm_r::dosimetry::PlayerDosimetryData` | Dose clamping (2500 max), zero passive decay, reset on death. |
| `com.hbm.items.tool.ItemGeigerCounter` | `nbm_r::geiger_hud` & `mixins/nbm_r/tick.rs` | 5-tick candidate pool, geiger1-geiger6 selection, needle dial, spatial audio. |
| `com.hbm.main.MainRegistry` | `data/nbm_r/horizon/item/hazmat_*.json` | Armor stats: `aMatHaz` (factor 60: 660, 960, 900, 780; armor 2, 5, 4, 1). |
| `com.hbm.potion.HbmPotion` | `data/ntm/horizon/mob_effect/*.json` | Content effect definitions + custom damage hooks. |

#### Detailed Hazard & Radiation Mechanics Parity

1. **Geiger Counter Audio Playback (`ItemGeigerCounter.java` -> `tick.rs`):**
   - **Original Java behavior:** Plays `hbm:item.geiger` with pitch jitter (`0.8F + itemRand.nextFloat() * 0.4F`) and sound tier selection based on radiation range.
   - **Horizon Native implementation:** Uses `game.native_play_local_sound("nbm_r:geiger<1..6>", [p.x, p.y, p.z], effective_vol, 1.0, 0)` with pitch fixed at 1.0 and volume scaled by `effective_audio_volume(1.0, master_volume, player_volume)`.
   - **Engine API rationale:** `game.player_menu_sound` is intentionally avoided because Horizon's `menu_clicks.rs` forcibly randomizes pitch (`0.8 + 0.4 * rand`) and overrides volume to `0.8`, corrupting Geiger click audio. `native_play_local_sound` gives crisp, unattenuated pitch and spatial realism.

2. **Radiation Sickness Status Effects (`EntityEffectHandler.java` -> `tick.rs`):**
   - **Original Java behavior:** Rolls RNG checks (`rand.nextInt(300) == 0`, `rand.nextInt(500) == 0`, `rand.nextInt(700) == 0`) each tick to apply vanilla potion effects (`confusion`, `moveSlowdown`, `weakness`, `poison`, `wither`).
   - **Horizon Native implementation:** Periodically refreshes active status effects every 40 ticks (2 seconds, starting on tick 1) via command dispatch:
     ```
     effect give @p <effect_id> <duration_sec> <amplifier> true
     ```
     The trailing `true` hides ambient potion particles, matching Java HBM's invisible debuff styling.
   - **Effect Tiers:**
     - Stage 1 (200 - 399 RAD): Nausea I (amplifier 0, 5s), Weakness I (amplifier 0, 5s).
     - Stage 2 (400 - 599 RAD): Nausea I (amplifier 0, 7s), Slowness I (amplifier 0, 5s), Weakness II (amplifier 1, 5s).
     - Stage 3 (600 - 799 RAD): Nausea I (amplifier 0, 7s), Slowness III (amplifier 2, 10s), Weakness III (amplifier 2, 10s), Poison II (amplifier 1, 3s).
     - Stage 4 (800 - 999 RAD): Nausea I (amplifier 0, 7s), Slowness III (amplifier 2, 10s), Weakness III (amplifier 2, 10s), Poison III (amplifier 2, 3s), Wither II (amplifier 1, 3s).

3. **Lethal Radiation Damage & Dose Reset (`EntityEffectHandler.java:237` -> `tick.rs`):**
   - **Original Java behavior:**
     ```java
     if (props.getRadiation() >= 1000.0F) {
         entity.attackEntityFrom(ModDamageSource.radiation, 1000F);
         props.setRadiation(0.0F);
     }
     ```
   - **Horizon Native implementation:** Evaluates lethal threshold (`dose >= 1000.0`). Resets accumulated dose via `set_player_dose(0, 0.0)` and executes lethal damage via `game.native_run_command("kill @p")`. Dose is cleared so respawned players start clean.

### Energy & Fluid Transport
| Original Java Class | Target Rust Implementation | Notes |
|---|---|---|
| `api.hbm.energymk2.IEnergyConnectorMK2` | `nbm_r::energy::ModEnergyBuffer` | Trait capability for machines and batteries. |
| `com.hbm.tileentity.conductor.TileEntityCable` | `nbm_r::energy::CableGrid` | BFS graph component ticking in `level_tick_pre`. |
| `com.hbm.inventory.fluid.FluidType` | `data/ntm/horizon/fluid/<name>.json` | Native fluid with viscosity/temperature parameters. |
| `com.hbm.inventory.fluid.tank.FluidTank` | `nbm_r::fluid::ModFluidTanks` | Multi-tank capability attached to machines. |

### Machinery & TileEntities
| Original Java Class | Target Rust Implementation | Notes |
|---|---|---|
| `com.hbm.tileentity.TileEntityMachineBase` | `nbm_r::machine::MachineBase` | Struct wrapping `ModBlockEntity` with slots & data. |
| `TileEntityMachineCentrifuge` | `nbm_r::machine::centrifuge` | Isotopes enrichment logic. |
| `TileEntityMachineBlastFurnace` | `nbm_r::machine::blast_furnace` | High-temperature metallurgy recipes. |
| `TileEntityMachineChemicalPlant` | `nbm_r::machine::chemplant` | Multi-fluid/solid chemical processing. |

### RBMK Reactor Components
| Original Java Class | Target Rust Implementation | Notes |
|---|---|---|
| `TileEntityRBMKConsole` | `nbm_r::rbmk::Console` | Central monitoring and SCRAM automation. |
| `TileEntityRBMKRod` | `nbm_r::rbmk::FuelColumn` | Neutron flux emission and burnup equations. |
| `TileEntityRBMKControl` | `nbm_r::rbmk::ControlColumn` | Variable neutron absorption depth. |
| `TileEntityRBMKBoiler` | `nbm_r::rbmk::BoilerColumn` | Thermodynamic heat-to-steam conversion. |
| `TileEntityRBMKModerator` | `nbm_r::rbmk::ModeratorColumn` | Fast-to-thermal neutron moderation factor. |

### Explosions & Ballistics
| Original Java Class | Target Rust Implementation | Notes |
|---|---|---|
| `com.hbm.explosion.ExplosionNukeGeneric` | `nbm_r::explosion::NuclearBlast` | Raymarched raycasting with block-hardness decay. |
| `com.hbm.render.entity.RenderNukeBlast` | `nbm_r::render::shader_blast` | WGSL shader hooked via `native_world_warp`. |
| `com.hbm.entity.missile.EntityMissileCustom` | `nbm_r::entity::Missile` | Ballistic trajectory entity updated in tick. |

---

## 3. Horizon Hook Points Lookup Table

| Horizon Hook | File Location | Intended NBM_R Usage |
|---|---|---|
| `native_hooks::Game::native_tick` | `mixins/nbm_r/tick.rs` | 4-step tick pipeline: (1) chunk diffusion/decay, (2) player exposure/dosimetry, (3) sickness evaluation & command dispatch, (4) Geiger audio candidate pool & spatial sound playback. |
| `native_hooks::Game::native_hud` | `mixins/nbm_r/hud.rs` | Geiger counter needle gauge, radiation meter, compass, missile radar. |
| `native_hooks::Game::native_key` | `mixins/nbm_r/key.rs` | Keybindings (suit night vision, Geiger sound mute). |
| `native_hooks::Game::native_settings` | `mixins/nbm_r/settings.rs` | In-game configuration menu (Options > Mods > NTM). |
| `native_hooks::Level::level_tick_post` | `mixins/nbm_r/level_tick.rs` | Radiation diffusion, cable grids, queued nuclear blasts. |
| `native_hooks::Player::player_tick_post`| `mixins/nbm_r/player_tick.rs`| Radiation exposure calculation, hazmat suit degradation. |
| `native_hooks::Block::block_entity_tick`| `mixins/nbm_r/machine_tick.rs`| Machine processing, RBMK reactor physics ticks. |
| `native_hooks::Render::world_drawn` | `mixins/nbm_r/render_world.rs`| Volumetric nuclear clouds, custom laser beams via wgpu. |
| `native_hooks::Render::native_world_warp`| `mixins/nbm_r/warp.rs`| Blast shockwave distortion WGSL shader. |
