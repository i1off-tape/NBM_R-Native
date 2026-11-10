# Instructions for AI Agents Working on NBM_R-Native

Welcome, agent. This document (`AGENTS.md`) is your primary operational manual for developing and maintaining **NBM_R-Native** — the high-performance native Rust port of **HBM's Nuclear Tech Mod (NTM)** for the **Horizon** voxel engine.

---

## 1. Environment & Path Map

All paths are relative to `/home/i1off/horizon_repos/`:

| Component | Path | Description |
|---|---|---|
| **This Mod (NBM_R-Native)** | `/home/i1off/horizon_repos/NBM_R-Native` | Your working directory. Contains mixins, runtime, configs. |
| **Original HBM Mod (Java)** | `/home/i1off/horizon_repos/hbm-s-nuclear-tech-git` | Reference source code of the original 1.7.10/Forge mod. |
| **Horizon Engine** | `/home/i1off/horizon_repos/horizon` | Game client/server source, native hooks definitions, docs. |
| **Horizon Mixins Tool** | `/home/i1off/horizon_repos/horizon-mixins` | AST transformation engine and CLI tool. |
| **Horizon Native Mods** | `/home/i1off/horizon_repos/horizon-mods` | Reference official native mods (`fps-overlay`, `recipes`, etc.). |
| **Rust / Cargo Binary** | `/home/i1off/.local/share/horizon-launcher/rust/1.99.1-x86_64-unknown-linux-gnu/bin` | Use this in `PATH` if `cargo` is not in standard system PATH. |

### Essential Environment Setup
Before running shell commands, always ensure Cargo is in your `PATH`:
```bash
export PATH="/home/i1off/.local/share/horizon-launcher/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
```

---

## 2. Inviolable Architectural Rules

1. **Strict Source Separation (Never modify Horizon or HBM in-place)**:
   - Never edit files in `/home/i1off/horizon_repos/horizon` directly.
   - Never commit build artifacts, binaries, or exported sources into Git.
   - All changes to the game engine MUST go through `mixins/nbm_r/horizon-mixin.toml` via `horizon-mixins`.
2. **Single Source as Library & Module**:
   - `mixins/nbm_r/runtime.rs` is compiled twice:
     1. Autonomously as a standard Cargo library crate (via `[lib].path = "mixins/nbm_r/runtime.rs"` in `Cargo.toml`) for instant unit testing (`cargo test`).
     2. Injected as `pub(crate) mod nbm_r` into Horizon's `crates/horizon-client/src/game.rs`.
   - **Do NOT add file-level doc comments (`//!`) or crate attributes (`#![...]`) in `runtime.rs`**! Injected submodules forbid outer file attributes (triggers `E_MODULE`). Use `//` line comments instead.
3. **Pure Rust Domain Logic (Zero-Horizon-Types in `runtime.rs`)**:
   - Keep core simulation algorithms (radiation dose decay, RBMK neutron flux, chemical formulas, explosion raymarching) as pure, side-effect-free functions taking primitive types (`f32`, `i32`, slices) or mod-local structs.
   - Connect to Horizon types only in thin glue helpers (`glue.rs` or specific hook files).
4. **Strict Cargo Dependency Policy**:
   - The root `Cargo.toml` has NO external dependencies.
   - Native mods can only use crates already pinned in Horizon's root `Cargo.lock`. Do not add arbitrary crates.io dependencies without verifying they exist in Horizon's workspace lockfile.
5. **Always Gate Hooks**:
   - Every hook helper MUST begin with:
     ```rust
     if !game.native_on(crate::game::nbm_r::ID) { return; }
     ```
6. **Workspace Test Isolation (Never run full `cargo test` on Horizon engine)**:
   - Do NOT run `cargo test` across `/home/i1off/horizon_repos/horizon/`. It compiles >100 workspace crates, takes minutes, and exhausts machine memory.
   - For verifying mixins and game compilation, use only `python3 tools/horizon.py check` and `python3 tools/test_template.py`.
   - Run unit tests exclusively in the mod directory via local `cargo test`.

---

## 3. How to Port a Feature from Java HBM to Rust

When assigned a task to port a subsystem (e.g., Radiation, Centrifuge, RBMK, Grenade):

### Step 1: Locate Original Java Code in `hbm-s-nuclear-tech-git`
Search the package structure in `/home/i1off/horizon_repos/hbm-s-nuclear-tech-git/src/main/java/com/hbm/`:
- `hazard/` -> Radiation, toxicity, explosion hazard data.
- `saveddata/` -> World-level chunk tracking.
- `tileentity/` -> Machine, reactor, and cable logic.
- `inventory/` -> Containers, slot rules, fluid tanks.
- `explosion/` -> Blast algorithms.

### Step 2: Decide Architecture: Data vs. Native Code
Horizon enforces a dual-package model:
1. **Content Mod (`.hmod`)**:
   - Holds declarative content: blocks, items, textures, sounds, language files, creative tabs (`data/`, `assets/`, `horizon-mod.toml`).
   - Installed in `~/.local/share/horizon/horizon-mods/<id>.hmod`.
   - Must contain only standard vanilla component types. Never use custom/NBT fields (e.g. `minecraft:custom_data`) in item definitions, as Horizon will abort loading the entire mod.
2. **Native Mod (`.hnmod`)**:
   - Holds AST mixin injections and pure Rust simulation logic (`mixins/<id>/`).
   - Installed in `~/.local/share/horizon/horizon-native-mods/<id>.hnmod`.
   - The launcher compiles this into the game client automatically upon startup.

### Step 3: Implement Pure Simulation & Tests
Write unit tests directly in `mixins/nbm_r/runtime.rs` or a submodule under `#[cfg(test)]`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_centrifuge_output() { ... }
}
```
Run tests immediately:
```bash
cargo test
```

### Step 4: Wire Hook Injections
Add the necessary `[[injection]]` in `mixins/nbm_r/horizon-mixin.toml` and write the thin helper file (e.g., `mixins/nbm_r/tick.rs`).

### Step 5: Validate and Package
Always run the validation suite before packaging or distribution:
```bash
export PATH="/home/i1off/.local/share/horizon-launcher/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
cargo test
python3 tools/validate_content.py
python3 tools/test_template.py
python3 tools/package.py
```
`python3 tools/package.py` validates content definitions and builds both artifacts in `target/`:
- `target/<id>-<version>.hnmod` (the native mixin bundle)
- `target/<id>-<version>.hmod` (the declarative content bundle)

---

## 4. Key Reference Documents in this Directory

- `PORTING_GUIDE.md`: Comprehensive, subsystem-by-subsystem migration roadmap.
- `MAPPING.md`: Exact mapping table between Java classes/packages and Rust targets.
- `compatibility.json`: Locked commit hashes of Horizon and horizon-mixins.


---

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical names, unchanged. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout at repo root. See `docs/agents/domain.md`.
