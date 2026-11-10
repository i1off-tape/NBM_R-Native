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
| `com.hbm.saveddata.RadiationSavedData` | `nbm_r::hazard::ChunkRadiationAttachment` | Server Attachment on `Holder::Chunk([x, z])`. |
| `com.hbm.items.tool.ItemGeigerCounter` | `nbm_r::hazard::geiger_hud` | Hook `native_hud` + audio playback. |
| `com.hbm.potion.HbmPotion` | `data/ntm/horizon/mob_effect/*.json` | Content effect definitions + custom damage hooks. |

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
| `native_hooks::Game::native_hud` | `mixins/nbm_r/hud.rs` | Geiger counter, radiation gauge, compass, missile radar. |
| `native_hooks::Game::native_key` | `mixins/nbm_r/key.rs` | Keybindings (suit night vision, Geiger sound mute). |
| `native_hooks::Game::native_settings` | `mixins/nbm_r/settings.rs` | In-game configuration menu (Options > Mods > NTM). |
| `native_hooks::Level::level_tick_post` | `mixins/nbm_r/level_tick.rs` | Radiation diffusion, cable grids, queued nuclear blasts. |
| `native_hooks::Player::player_tick_post`| `mixins/nbm_r/player_tick.rs`| Radiation exposure calculation, hazmat suit degradation. |
| `native_hooks::Block::block_entity_tick`| `mixins/nbm_r/machine_tick.rs`| Machine processing, RBMK reactor physics ticks. |
| `native_hooks::Render::world_drawn` | `mixins/nbm_r/render_world.rs`| Volumetric nuclear clouds, custom laser beams via wgpu. |
| `native_hooks::Render::native_world_warp`| `mixins/nbm_r/warp.rs`| Blast shockwave distortion WGSL shader. |
