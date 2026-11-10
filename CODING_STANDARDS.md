# Coding Standards for NBM_R-Native

This document defines the coding and architectural standards enforced during code reviews for **NBM_R-Native**.

---

## 1. Declarative Content & Data (`data/` and `assets/`)

1. **Standard Item Components Only**:
   - Item definitions in `data/<id>/horizon/item/*.json` must strictly use supported vanilla components (e.g. `minecraft:equippable`, `minecraft:max_damage`, `minecraft:attribute_modifiers`).
   - **Never use `minecraft:custom_data` or arbitrary NBT fields** in item declarations. Horizon item tables do not include custom data by default; declaring them causes the engine loader to abort mod loading with `is not a known component type`.
   - Mod logic identifying radioactive items or diagnostic tools must query item identifiers (`&str`) in Rust rather than reading component metadata.
2. **Deterministic Content Packaging**:
   - Every new item, block, model, texture, and creative tab must pass validation via `python3 tools/validate_content.py`.
   - Content mods must declare their namespaces in `horizon-mod.toml` (e.g. `namespaces = ["ntm"]`).

---

## 2. Rust Simulation Code (`mixins/nbm_r/runtime.rs`)

1. **Pure Domain Logic**:
   - Core simulation math (radiation decay, mass conservation diffusion, dosimetry, sickness progression, Geiger audio rate mapping) must remain pure, deterministic, and free of Horizon game engine types.
   - All public calculations must be accompanied by comprehensive unit tests under `#[cfg(test)]`.
2. **Test Concurrency Safety**:
   - Any test that modifies global static trackers (`reset_state()`, `PLAYER_TRACKER`, `TRACKER`) must acquire the test synchronization lock (`TEST_MUTEX`) to prevent race conditions during multi-threaded `cargo test`.
3. **No Code Duplication in Hooks**:
   - Calculations reused across multiple hook points (such as calculating inventory radiation rate in `tick.rs` and `hud.rs`) must be centralized as helper functions in `runtime.rs` (e.g. `total_item_radiation`).

---

## 3. Mixin Hook Injections (`mixins/nbm_r/*.rs`)

1. **Mandatory Mod State Gate**:
   - Every hook helper must immediately check:
     ```rust
     if !game.native_on(crate::game::nbm_r::ID) { return; }
     ```
2. **Thin Glue Discipline**:
   - Hook helper files (`break.rs`, `tick.rs`, `hud.rs`) must serve solely as thin glue between Horizon engine APIs and `crate::game::nbm_r::*`.
   - Never embed complex algorithmic simulation directly inside hook files.

---

## 4. Terminology & Units

- Radiation rate must be expressed in **Rad/s** (or "radiation rate"), never vague terms like "intensity".
- Accumulated dose must be expressed in **Rad**.
- Activity / contamination must follow HBM units and GLOSSARY definitions.
