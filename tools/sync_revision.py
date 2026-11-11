#!/usr/bin/env python3
"""Sync Horizon revision and game.rs SHA256 across compatibility.json and horizon-mixin.toml."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_HORIZON = ROOT.parent / "horizon"


def get_git_commit(repo: Path, ref: str) -> str:
    res = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", ref],
        capture_output=True,
        text=True,
        check=True,
    )
    return res.stdout.strip()


def get_file_bytes_at_rev(repo: Path, rev: str, rel_path: str) -> bytes:
    res = subprocess.run(
        ["git", "-C", str(repo), "show", f"{rev}:{rel_path}"],
        capture_output=True,
        check=True,
    )
    return res.stdout


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sync_revision(horizon_repo: Path, target_rev: str | None = None) -> None:
    if not (horizon_repo / ".git").exists():
        print(f"Error: Horizon repository not found at {horizon_repo}", file=sys.stderr)
        sys.exit(1)

    ref = target_rev if target_rev else "HEAD"
    full_commit = get_git_commit(horizon_repo, ref)
    print(f"Target revision: {full_commit} (from ref: '{ref}')")

    target_file = "crates/horizon-client/src/game.rs"
    file_bytes = get_file_bytes_at_rev(horizon_repo, full_commit, target_file)
    file_sha256 = digest(file_bytes)
    print(f"Computed SHA256 for {target_file}: {file_sha256}")

    # 1. Update compatibility.json
    compat_path = ROOT / "compatibility.json"
    compat_data = json.loads(compat_path.read_text())
    old_rev = compat_data.get("horizon_revision", "")
    compat_data["horizon_revision"] = full_commit
    compat_data["source_sha256"][target_file] = file_sha256
    compat_path.write_text(json.dumps(compat_data, indent=2) + "\n")
    print(f"Updated {compat_path.relative_to(ROOT)} ({old_rev[:10]} -> {full_commit[:10]})")

    # 2. Update mixins/nbm_r/horizon-mixin.toml
    mixin_path = ROOT / "mixins/nbm_r/horizon-mixin.toml"
    mixin_text = mixin_path.read_text()
    pattern = r'(horizon_revision\s*=\s*")[^"]*(")'
    new_mixin_text = re.sub(pattern, rf'\g<1>{full_commit}\g<2>', mixin_text)
    if new_mixin_text != mixin_text:
        mixin_path.write_text(new_mixin_text)
        print(f"Updated {mixin_path.relative_to(ROOT)}")
    else:
        print(f"Already up to date in {mixin_path.relative_to(ROOT)}")

    print("\nRevision sync complete.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--horizon",
        type=Path,
        default=DEFAULT_HORIZON,
        help=f"Path to Horizon git repository (default: {DEFAULT_HORIZON})",
    )
    parser.add_argument(
        "--rev",
        type=str,
        default=None,
        help="Target commit or ref (defaults to HEAD of the Horizon repository)",
    )
    args = parser.parse_args()
    sync_revision(args.horizon, args.rev)


if __name__ == "__main__":
    main()
