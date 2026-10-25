# Legacy asset support

## Current commands
- `cargo run -p openem4 --bin em4-inspect -- path/to/file.v3o`: non-mutating file inspection.
- `cargo run -p openem4 --bin em4-texture -- path/to/texture.dds preview.png`: decode DXT1 / DXT3 / DXT5 DDS first mip into an RGBA PNG file.

### DDS
The CPU preview decoder implements BC1 / BC2 / BC3 with bounds checking, edge cropping and explicit unsupported-format errors. It currently does not decode uncompressed DDS, cubemaps, DX10 DDS, array textures, or later mip levels.

### V3O
Original EM4 V3O geometry is normally packed binary and the game's Editor supports unpacking to ASCII. A reliable binary format specification and compatible nonredistributable test fixtures are still needed. The inspector only classifies potential text versus binary; it does not decode geometry. Never unpack the original installation in place—copy files first.

### E4P / E4M
These remain opaque and unparsed.

Source: https://github.com/em4resource/EM4-Modding-Encyclopedia/blob/main/docs/editor-manual-reference.md
