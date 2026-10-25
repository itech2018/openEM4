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

## V3O texture-reference research tool

Run `cargo run -p openem4 --bin em4-v3o-textures -- "C:\\path\\to\\unpacked.v3o"` on an unpacked **text** V3O copy. This performs a best-effort scan of DDS/TGA/PNG/BMP references; it does **not** decode packed meshes or validate the complete V3O grammar. The scanner rejects binary text and unsafe paths and does not alter files. Original editor unpack operations overwrite their targets; work only on copies.

## Packed V3O (working parser)
The original sample `asf.V3O` has little-endian magic `0x1337C0D3`, pack-version `1`, a decompressed byte length at offset 8 and zlib data beginning at offset 20. The new `v3o_packed::unpack()` decodes the file in memory, bounds the output to 64 MiB, verifies declared length and checks the `[VNUM=` marker. The original file remains untouched. The same `em4-v3o-textures` command now accepts supported packed models. Geometry parsing and GPU rendering remain future tasks. No user-supplied game asset is committed.
