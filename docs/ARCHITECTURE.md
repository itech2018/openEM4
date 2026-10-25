# Architecture

The openEM4 Cargo workspace separates application lifecycle, native GPU rendering, core entity types and read-only game installation access.

- `em4-core`: identifiers and basic transforms.
- `em4-vfs`: directory mounting, path resolution, mod priority and inventory scanning.
- `em4-render`: wgpu-based GPU shell and diagnostic grid.
- `openem4`: command-line application and event loop.

Future modules: legacy file formats, asset cache, simulation, scripting API, multiplayer, editor and launcher.

The v0.1 renderer is a development diagnostic and is not a map renderer. Original gameplay and mod compatibility are longer-term goals.
