# Compatibility

The goal is to use original Emergency 4 content and existing mods unchanged, without copying or redistributing their proprietary assets.

## Levels
1. Recognize and read legacy files.
2. Resolve mod content and file overrides.
3. Recreate scripting APIs and gameplay semantics.
4. Verify complete mods in end-to-end gameplay.

v0.1 scans file extensions and mounts directories read-only; it does **not** parse V3O, E4P or E4M content. Unknown content must produce actionable errors, not silent success.

Native binary plugins are a separate compatibility risk.
