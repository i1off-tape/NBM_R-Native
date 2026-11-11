# Instructions for AI Agents Working on NBM_R-Native

Welcome, agent. This document (`AGENTS.md`) is your primary operational manual for developing and maintaining **NBM_R-Native** — the high-performance native Rust port of **HBM's Nuclear Tech Mod (NTM)** for the **Horizon** voxel engine.


READ: README.MD , VALIDATION.MD , GLOSSARY.MD, CODING_STANDARTS.MD

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
All Python tools in `tools/` (`check.py`, `horizon.py`, `test_template.py`) automatically detect and prepend the Horizon Cargo toolchain to `PATH`. If running standalone shell commands (like raw `cargo test`), ensure Cargo is in your `PATH`:
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

### Mandatory Knowledge Graph Usage (`codebase-memory-mcp`)
All three major repositories in `/home/i1off/horizon_repos/` are indexed in `codebase-memory-mcp`:
1. **This Mod:** `home-i1off-horizon_repos-NBM_R-Native` (or `/home/i1off/horizon_repos/NBM_R-Native`)
2. **Horizon Engine:** `home-i1off-horizon_repos-horizon` (or `/home/i1off/horizon_repos/horizon`)
3. **Java HBM Mod:** `home-i1off-horizon_repos-hbm-s-nuclear-tech-git` (or `/home/i1off/horizon_repos/hbm-s-nuclear-tech-git`)

**CRITICAL REQUIREMENT:**
- **ALWAYS** query `codebase-memory-mcp` (`search_graph`, `get_code_snippet`, `trace_path`, `query_graph`) first before attempting any grep/file searching.
- **NEVER** run broad, unguided `git grep` or `find` loops across `horizon` or `hbm-s-nuclear-tech-git`. It is slow, exhausts context, and causes delays.
- Pass the absolute repository path (or exact project name) as `project` to MCP tools. Fall back to grep ONLY for raw string literals, configs, or non-code assets (`sounds.json`, `.toml`).

---

## 3. How to Port a Feature from Java HBM to Rust

When assigned a task to port a subsystem (e.g., Radiation, Centrifuge, RBMK, Grenade):

### Step 1: Locate Original Java Code in `hbm-s-nuclear-tech-git`
Query `codebase-memory-mcp` with `project = "/home/i1off/horizon_repos/hbm-s-nuclear-tech-git"`.

Package structure in `/home/i1off/horizon_repos/hbm-s-nuclear-tech-git/src/main/java/com/hbm/`:
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
**Diagnostic Loop Discipline**: Once the root cause is identified during a diagnostic loop, proceed directly to writing a reproduction test or applying the minimal fix; avoid redundant exploratory reads of engine sources.

### Step 4: Wire Hook Injections
Add the necessary `[[injection]]` in `mixins/nbm_r/horizon-mixin.toml` and write the thin helper file (e.g., `mixins/nbm_r/tick.rs`).

### Step 5: Validate and Package
Always run the validation suite before packaging or distribution:
```bash
export PATH="/home/i1off/.local/share/horizon-launcher/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
python3 tools/check.py
python3 tools/horizon.py check
python3 tools/package.py
```
`python3 tools/check.py` is the unified fast check runner (executing `cargo test`, mixin AST template checks, and content invariants).
`python3 tools/package.py` validates content definitions and builds both artifacts in `target/`:
- `target/<id>-<version>.hnmod` (the native mixin bundle)
- `target/<id>-<version>.hmod` (the declarative content bundle)

---

## 4. Game Struct Contract for Mixin Hooks

Mixin hook helper files (such as `tick.rs`, `hud.rs`) interact with `&mut Game<'_>`. To maintain compatibility with both the Horizon engine and the template compiler (`tools/test_template.py`), only access the following sanctioned fields and methods:

### Accessible Public Fields on `Game`:
- `game.positions: &'a mut Vec<[i32; 3]>`: list of block positions.
- `game.inventory: Vec<Option<ItemStack>>`: main inventory slots (0..36).
- `game.selected: usize`: currently selected hotbar slot index (0..9).
- `game.armor: [Option<ItemStack>; 4]`: equipped armor pieces (head, chest, legs, feet).
- `game.offhand: Option<ItemStack>`: equipped offhand slot item.
- `game.settings: Settings`: settings with `master_volume: f32` and `volumes: [f32; 11]`.

### Accessible Public Methods on `Game`:
- `game.native_on(id: &str) -> bool`: check if mod is enabled.
- `game.native_window() -> bool`: check if running with active window (client side).
- `game.native_player() -> Option<NativePlayer>`: client-side player coordinates (`x`, `y`, `z`, `yaw`, `pitch`, `dimension`).
- `game.native_acting_player() -> Option<NativeServerPlayer>`: server-side acting player reference.
- `game.native_server_players() -> Vec<NativeServerPlayer>`: authoritative list of server players (`uuid`, `name`, `health`, `position`, `dimension`).
- `game.native_player_stacks(uuid: u128) -> Option<Vec<Option<ItemStack>>>`: inventory stacks of a server player.
- `game.native_player_equipment(uuid: u128, slot: EquipmentSlot) -> Option<ItemStack>`: equipped armor or held item of a server player.
- `game.native_run_command(cmd: &str) -> bool`: execute authoritative command on server (`effect`, `kill`).
- `game.native_chat(msg: &str)`: send local chat message to player.
- `game.player_menu_sound(sound_id: &str)`: play non-spatial UI audio track.
- `game.native_play_local_sound(sound_id, at, volume, pitch, delay)`: play spatial sound.
- `game.native_corner_shown(corner: Corner) -> bool`: check HUD corner visibility.
- `game.native_fill(batch, rect, argb)`: draw 2D rectangle on HUD.
- `game.native_text(batch, text, at, color, shadow)`: render text string on HUD.
- `game.native_text_width(text: &str) -> f32`: measure HUD text width.
---

## 5. Key Reference Documents in this Directory

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
