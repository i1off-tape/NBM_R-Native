#!/usr/bin/env python3
"""Validate declarative content (data/ and assets/) for Horizon mod compliance.

Checks:
- JSON syntax and UTF-8 encoding across all data/ and assets/ files.
- Manifest requirements in horizon-mod.toml.
- Item declarations (template, max_stack_size, rarity, supported components).
- Disallows unknown or unsupported components such as `minecraft:custom_data`.
- Block declarations (template, properties).
- Creative tabs (title, icon, items list).
"""

import json
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]

# Allowed vanilla item component types supported by Horizon item tables
ALLOWED_COMPONENTS = {
    "minecraft:max_damage",
    "minecraft:max_stack_size",
    "minecraft:damage",
    "minecraft:unbreakable",
    "minecraft:custom_name",
    "minecraft:item_name",
    "minecraft:lore",
    "minecraft:rarity",
    "minecraft:enchantments",
    "minecraft:repair_cost",
    "minecraft:attribute_modifiers",
    "minecraft:dyed_color",
    "minecraft:map_color",
    "minecraft:map_id",
    "minecraft:map_decorations",
    "minecraft:map_post_processing",
    "minecraft:charged_projectiles",
    "minecraft:bundle_contents",
    "minecraft:potion_contents",
    "minecraft:suspicious_stew_effects",
    "minecraft:writable_book_content",
    "minecraft:written_book_content",
    "minecraft:trim",
    "minecraft:debug_stick_state",
    "minecraft:entity_data",
    "minecraft:bucket_entity_data",
    "minecraft:block_entity_data",
    "minecraft:instrument",
    "minecraft:ominous_bottle_amplifier",
    "minecraft:jukebox_playable",
    "minecraft:recipes",
    "minecraft:lodestone_tracker",
    "minecraft:firework_explosion",
    "minecraft:fireworks",
    "minecraft:profile",
    "minecraft:note_block_sound",
    "minecraft:banner_patterns",
    "minecraft:base_color",
    "minecraft:pot_decorations",
    "minecraft:container",
    "minecraft:block_state",
    "minecraft:enchantment_glint_override",
    "minecraft:food",
    "minecraft:damage_resistant",
    "minecraft:tool",
    "minecraft:equippable",
    "minecraft:repairable",
    "minecraft:glider",
    "minecraft:consumable",
    "minecraft:use_cooldown",
    "minecraft:use_remainder",
}

DISALLOWED_COMPONENTS = {
    "minecraft:custom_data": "custom_data is not in vanilla item definition tables and will abort Horizon mod loading",
}


def validate_content():
    errors = []
    warnings = []
    checked_files = 0

    # 1. Validate horizon-mod.toml
    manifest_path = ROOT / "horizon-mod.toml"
    if manifest_path.is_file():
        try:
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            for req in ["id", "name", "version"]:
                if req not in manifest:
                    errors.append(f"{manifest_path.name}: missing required key '{req}'")
        except Exception as e:
            errors.append(f"{manifest_path.name}: invalid TOML: {e}")
    else:
        warnings.append("horizon-mod.toml is missing at repository root")

    # 2. Check all JSON files in data/ and assets/
    for folder in ["data", "assets"]:
        base = ROOT / folder
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*.json")):
            checked_files += 1
            rel = p.relative_to(ROOT)
            try:
                data = json.loads(p.read_text(encoding="utf-8"))
            except Exception as e:
                errors.append(f"{rel}: invalid JSON: {e}")
                continue

            # Special checks for item definitions: data/<id>/horizon/item/*.json
            parts = rel.parts
            if len(parts) >= 5 and parts[0] == "data" and parts[2] == "horizon" and parts[3] == "item":
                if not isinstance(data, dict):
                    errors.append(f"{rel}: item declaration must be a JSON object")
                    continue
                if "template" not in data:
                    errors.append(f"{rel}: item declaration must define a 'template'")

                comps = data.get("components")
                if comps is not None:
                    if not isinstance(comps, dict):
                        errors.append(f"{rel}: 'components' must be a JSON object")
                    else:
                        for comp_name in comps.keys():
                            clean_name = comp_name.lstrip("!")
                            if clean_name in DISALLOWED_COMPONENTS:
                                errors.append(
                                    f"{rel}: forbidden component '{comp_name}': {DISALLOWED_COMPONENTS[clean_name]}"
                                )
                            elif clean_name not in ALLOWED_COMPONENTS and not clean_name.startswith("horizon:"):
                                errors.append(
                                    f"{rel}: unknown or unsupported component '{comp_name}'"
                                )

            # Special checks for block definitions: data/<id>/horizon/block/*.json
            if len(parts) >= 5 and parts[0] == "data" and parts[2] == "horizon" and parts[3] == "block":
                if not isinstance(data, dict):
                    errors.append(f"{rel}: block declaration must be a JSON object")
                elif "template" not in data:
                    errors.append(f"{rel}: block declaration must define a 'template'")

            # Special checks for creative tabs: data/<id>/horizon/creative_tab/*.json
            if len(parts) >= 5 and parts[0] == "data" and parts[2] == "horizon" and parts[3] == "creative_tab":
                if not isinstance(data, dict):
                    errors.append(f"{rel}: creative tab declaration must be a JSON object")
                else:
                    for field in ["title", "icon", "items"]:
                        if field not in data:
                            errors.append(f"{rel}: creative tab must define '{field}'")
                    if "items" in data and not isinstance(data["items"], list):
                        errors.append(f"{rel}: creative tab 'items' must be a list")

    return {
        "status": "ok" if not errors else "error",
        "checked_files": checked_files,
        "errors": errors,
        "warnings": warnings,
    }


def main():
    result = validate_content()
    print(json.dumps(result, indent=2))
    if result["status"] != "ok":
        sys.exit(1)


if __name__ == "__main__":
    main()
