#!/usr/bin/env python3
"""Package the mod as a source-only `.hnmod`, the file Horizon's registry and launcher take.

The archive holds `horizon-native.json`, `horizon-mod.toml`, `assets/`, `data/`,
and `mixins/<id>/` (with deterministic timestamps and permissions), so the same
sources always give the same bytes and the same SHA-256.
"""
import hashlib
import json
from pathlib import Path
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
GAME = '26.3'


def package():
    compat = json.loads((ROOT / 'compatibility.json').read_text())
    manifests = sorted((ROOT / 'mixins').glob('*/horizon-mixin.toml'))
    if len(manifests) != 1:
        raise SystemExit('a .hnmod holds one bundle: keep one mixins/<id>/horizon-mixin.toml')
    bundle = manifests[0].parent
    mixin = tomllib.loads(manifests[0].read_text())
    mod = mixin['mod']
    if bundle.name != mod['id']:
        raise SystemExit(f"rename mixins/{bundle.name}/ to mixins/{mod['id']}/ (the registry finds it by id)")
    if not mod['id'].replace('_', '').isalnum() or not mod['id'].islower():
        raise SystemExit('the registry takes ids of lowercase letters, digits and _ only')
    if mixin.get('compatibility', {}).get('horizon_revision') != compat['horizon_revision']:
        raise SystemExit('horizon-mixin.toml and compatibility.json pin different Horizon revisions')

    # Collect content files (assets, data, horizon-mod.toml)
    content_files = {}
    if (ROOT / 'horizon-mod.toml').is_file():
        content_files['horizon-mod.toml'] = (ROOT / 'horizon-mod.toml').read_bytes()
    for top in ['assets', 'data']:
        dir_path = ROOT / top
        if dir_path.is_dir():
            for p in sorted(dir_path.rglob('*')):
                if p.is_file():
                    content_files[p.relative_to(ROOT).as_posix()] = p.read_bytes()

    # Build deterministic embedded content .hmod in mixins/<id>/data/
    if content_files:
        embedded_hmod = bundle / 'data' / f"{mod['id']}-content.hmod"
        embedded_hmod.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(embedded_hmod, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, content in sorted(content_files.items()):
                info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, content)

    # Prepare bundle files
    manifest = dict(schema=1, id=mod['id'], version=mod['version'], game=GAME,
                    horizon=compat['horizon_revision'], mixins=compat['horizon_mixins_revision'])
    files = {'horizon-native.json': (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()}

    for name, content in content_files.items():
        files[name] = content

    for p in sorted(bundle.rglob('*')):
        if p.is_file():
            rel = p.relative_to(bundle).as_posix()
            parts = rel.split('/')
            allowed = (p.suffix in ('.rs', '.toml') or
                       (len(parts) >= 2 and parts[0] == 'data') or
                       p.name in ('README', 'LICENSE', 'LICENCE', 'COPYING', 'NOTICE', '.gitignore'))
            if allowed:
                files[p.relative_to(ROOT).as_posix()] = p.read_bytes()

    out = ROOT / 'target' / f"{mod['id']}-{mod['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)

    print(json.dumps(dict(file=str(out), size=out.stat().st_size,
                          sha256=hashlib.sha256(out.read_bytes()).hexdigest()), indent=2))


if __name__ == '__main__':
    package()
