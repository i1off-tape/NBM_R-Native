# NBM_R-Native: HBM's Nuclear Tech Mod on Rust for Horizon

[![Rust](https://img.shields.io/badge/rust-1.95.0%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Engine](https://img.shields.io/badge/horizon-26.3-green.svg)](https://github.com/horizon-rust/horizon)

**NBM_R-Native** is the native, high-performance port of the legendary **HBM's Nuclear Tech Mod (NTM)**, engineered in Rust specifically for the **Horizon** voxel engine.

Unlike Java-based modding, NBM_R-Native is built as an **AOT-compiled native mod (`.hnmod`)** using `horizon-mixins`, achieving zero garbage collection pauses, SIMD acceleration, and modern GPU rendering with `wgpu`.

---

## 🧭 Documentation Quick Reference

For developers and AI agents working on this project:
- 📖 [**AGENTS.md**](file:///home/i1off/horizon_repos/NBM_R-Native/AGENTS.md) — Mandatory operational guide, safety invariants, and execution procedures for AI coding agents.
- 📐 [**PORTING_GUIDE.md**](file:///home/i1off/horizon_repos/NBM_R-Native/PORTING_GUIDE.md) — Comprehensive technical roadmap and subsystem architecture (Radiation, Energy, RBMK, Explosions).
- 🗺️ [**MAPPING.md**](file:///home/i1off/horizon_repos/NBM_R-Native/MAPPING.md) — Detailed mapping matrix translating Java classes (`com.hbm.*`) to Horizon Native Rust.

---

## 📁 Repository Layout

```text
NBM_R-Native/
├── AGENTS.md                  # Instructions & guardrails for AI agents
├── PORTING_GUIDE.md           # Master porting roadmap & engineering specs
├── MAPPING.md                 # Java-to-Rust class & package dictionary
├── README.md                  # Project overview & quickstart
├── Cargo.toml                 # Standalone Cargo package for instant testing
├── compatibility.json         # Pin hashes for Horizon & horizon-mixins
├── mixins/
│   └── nbm_r/
│       ├── horizon-mixin.toml # Injections & module registrations
│       ├── runtime.rs         # Pure Rust domain simulation & unit tests
│       └── break.rs           # Sample hook adapter
└── tools/
    ├── horizon.py             # Build, check, test, stage against Horizon
    ├── package.py             # Reproducible .hnmod packager
    ├── test_template.py       # Standalone AST mixin integration test
    └── audit/                 # AST invariant analyzer
```

---

## ⚡ Quickstart

### Prerequisites
- Python 3.12+
- Rust 1.95.0+ (Nightly toolchain provided by Horizon Launcher)

Set up your environment:
```bash
export PATH="/home/i1off/.local/share/horizon-launcher/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH"
```

### 1. Run Unit Tests (Pure Rust)
Executes instant tests in `runtime.rs` without needing the Horizon engine:
```bash
cargo test
```

### 2. Verify AST Mixins & Invariant Audit
Validates that AST patches cleanly integrate into Horizon's syntax tree:
```bash
python3 tools/test_template.py
```

### 3. Build Client with NBM_R-Native
Compile the mod directly into Horizon's client binary:
```bash
python3 tools/horizon.py build --horizon /home/i1off/horizon_repos/horizon
```

### 4. Package Release `.hnmod`
Produces a deterministic, byte-reproducible archive in `target/`:
```bash
python3 tools/package.py
```

---

## 🔗 Related Repositories

- **Original HBM Mod (Java)**: [`/home/i1off/horizon_repos/hbm-s-nuclear-tech-git`](file:///home/i1off/horizon_repos/hbm-s-nuclear-tech-git)
- **Horizon Engine**: [`/home/i1off/horizon_repos/horizon`](file:///home/i1off/horizon_repos/horizon)
- **Horizon Mixins**: [`/home/i1off/horizon_repos/horizon-mixins`](file:///home/i1off/horizon_repos/horizon-mixins)
- **Official Horizon Mods**: [`/home/i1off/horizon_repos/horizon-mods`](file:///home/i1off/horizon_repos/horizon-mods)
