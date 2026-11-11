#!/usr/bin/env python3
"""Unified fast verification suite for NBM_R-Native.

Runs:
1. Environment & Revision Compatibility check (compatibility.json vs horizon-mixin.toml vs local Horizon)
2. `cargo test` (unit tests in runtime.rs)
3. `test_template.py` (mixin AST check, template compilation for tick.rs/hud.rs, content validation)
"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import horizon

ROOT = Path(__file__).resolve().parents[1]


def ensure_env():
    horizon.ensure_cargo_in_path()
    return os.environ.copy()


def check_revision_compatibility():
    print("==> [0. Environment & Revision Compatibility] Checking...")
    compat_path = ROOT / "compatibility.json"
    mixin_path = ROOT / "mixins/nbm_r/horizon-mixin.toml"
    if not compat_path.exists():
        print(f"FAILED: {compat_path} missing!", file=sys.stderr)
        sys.exit(1)
    if not mixin_path.exists():
        print(f"FAILED: {mixin_path} missing!", file=sys.stderr)
        sys.exit(1)

    compat = json.loads(compat_path.read_text())
    pinned_rev = compat.get("horizon_revision", "")

    # Parse horizon_revision from mixin toml
    mixin_content = mixin_path.read_text()
    match = re.search(r'horizon_revision\s*=\s*"([^"]+)"', mixin_content)
    if not match:
        print(f"FAILED: horizon_revision not found in {mixin_path}", file=sys.stderr)
        sys.exit(1)
    mixin_rev = match.group(1)

    if pinned_rev != mixin_rev:
        print(
            f"FAILED: Revision mismatch between compatibility.json ({pinned_rev}) "
            f"and horizon-mixin.toml ({mixin_rev})!\n"
            f"Run `python3 tools/sync_revision.py --rev {pinned_rev}` to fix.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Optional local horizon git check
    horizon_repo = ROOT.parent / "horizon"
    if (horizon_repo / ".git").exists():
        # Check if pinned_rev exists in the repo
        ret = subprocess.run(
            ["git", "-C", str(horizon_repo), "cat-file", "-e", f"{pinned_rev}^{{commit}}"],
            capture_output=True,
        )
        if ret.returncode != 0:
            print(
                f"FAILED: Pinned revision {pinned_rev} does not exist in {horizon_repo}!",
                file=sys.stderr,
            )
            sys.exit(1)

        # Check sha256 of game.rs at pinned revision
        expected_sha = compat.get("source_sha256", {}).get("crates/horizon-client/src/game.rs")
        if expected_sha:
            show_res = subprocess.run(
                ["git", "-C", str(horizon_repo), "show", f"{pinned_rev}:crates/horizon-client/src/game.rs"],
                capture_output=True,
            )
            if show_res.returncode == 0:
                actual_sha = hashlib.sha256(show_res.stdout).hexdigest()
                if actual_sha != expected_sha:
                    print(
                        f"FAILED: SHA256 mismatch for crates/horizon-client/src/game.rs at {pinned_rev}!\n"
                        f"Expected: {expected_sha}\nActual:   {actual_sha}",
                        file=sys.stderr,
                    )
                    sys.exit(1)

        # Advisory check: is local HEAD different?
        head_rev = subprocess.run(
            ["git", "-C", str(horizon_repo), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
        ).stdout.strip()
        if head_rev and head_rev != pinned_rev:
            print(f"    [INFO] Local horizon HEAD is {head_rev[:10]} (pinned is {pinned_rev[:10]}).")

    print(f"PASSED: [0. Environment & Revision Compatibility] Pinned at {pinned_rev[:10]}\n")


def check_terminology():
    print("==> [0.5. Terminology Linter] Checking for forbidden radiation terms...")
    banned_patterns = [
        (re.compile(r'\b(intensit(y|ies))\b', re.IGNORECASE), "Use 'radiation rate' / 'Rad/s' instead of 'intensity'"),
        (re.compile(r'\b(roentgen)\b', re.IGNORECASE), "Use 'Rad' / 'Rad/s' instead of 'Roentgen'"),
        (re.compile(r'\b(flux/s)\b', re.IGNORECASE), "Use 'Rad/s' instead of 'Flux/s'"),
    ]

    scan_dirs = [ROOT / "mixins"]
    errors = []

    for scan_dir in scan_dirs:
        for p in scan_dir.rglob("*.rs"):
            lines = p.read_text(encoding="utf-8").splitlines()
            for idx, line in enumerate(lines, start=1):
                # Ignore comment lines in CODING_STANDARDS.md itself if scanned
                for pattern, hint in banned_patterns:
                    match = pattern.search(line)
                    if match:
                        errors.append(f"{p.relative_to(ROOT)}:{idx}: Found forbidden term '{match.group(0)}' ({hint})")

    if errors:
        print("FAILED: Forbidden terminology detected:", file=sys.stderr)
        for err in errors:
            print(f"  {err}", file=sys.stderr)
        print("See CODING_STANDARDS.md §4 and GLOSSARY.md for required terms.", file=sys.stderr)
        sys.exit(1)

    print("PASSED: [0.5. Terminology Linter] No forbidden terms found.\n")


def run_step(name, cmd, cwd=ROOT):
    print(f"==> [{name}] Running: {' '.join(cmd)}")
    env = ensure_env()
    proc = subprocess.run(cmd, cwd=cwd, env=env, text=True)
    if proc.returncode != 0:
        print(f"FAILED: [{name}] exited with code {proc.returncode}", file=sys.stderr)
        sys.exit(proc.returncode)
    print(f"PASSED: [{name}]\n")


def main():
    print("=== NBM_R-Native Verification Suite ===\n")
    check_revision_compatibility()
    check_terminology()
    run_step("1. Runtime Unit Tests", ["cargo", "test", "--quiet"])
    run_step("2. Mixin Compilation & Content Validation", [sys.executable, "tools/test_template.py"])
    print("All checks passed successfully.")


if __name__ == "__main__":
    main()
