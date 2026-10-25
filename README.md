# openEM4

Independent modern 64-bit Rust/wgpu engine project targeting compatibility with Emergency 4 / 911: First Responders content and community mods.

**Status:** v0.1.0-dev foundation. This is not yet a playable Emergency 4 replacement. No original copyrighted game files are included.

## Build

Install stable Rust from rustup.rs, then:

```sh
cargo run -p openem4 -- --help
cargo run -p openem4
cargo run -p openem4 -- --scan "C:\Games\Emergency 4" --headless
cargo test --workspace
```

Running the app displays a GPU diagnostic grid. The `--scan` option prints JSON inventory and discovered Mods folders; it does not modify original files.

See [architecture](docs/ARCHITECTURE.md), [compatibility](docs/COMPATIBILITY.md) and [roadmap](docs/ROADMAP.md).

MIT licensed, independent and unofficial. Don't redistribute proprietary game content.

## Format inspection (v0.2 research)

```
cargo run -p openem4 --bin em4-inspect -- "C:\\Games\\Emergency 4\\Mods\\MyMod\\Models\\truck.v3o"
```

This currently identifies packed/text V3O candidates, opaque E4P/E4M files and DDS headers; geometry decoding and model rendering are not yet implemented. See `docs/FORMATS.md`.

## DDS texture preview

`cargo run -p openem4 --bin em4-texture -- "C:\\path\\texture.dds" preview.png`

Supports DXT1, DXT3 and DXT5 (first mip); see `docs/FORMATS.md`. Packed V3O mesh parsing is still research work.

## 3D V3O viewer (experimental)

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Models\\Vehicles\\TEC\\asf.V3O"
```

Mouse controls: **left-drag** orbit, **right-drag** pan, **wheel** zoom, **R** reset. This loads supported packed V3O geometry and renders shaded, untextured triangles with a depth buffer. Material textures, normal correctness, UV scaling, orientation and full model compatibility still require verification. The original file is only read; never changed.

## Textured V3O viewer (experimental)

The existing `em4-view` command now looks for referenced textures alongside the V3O file (case-insensitive filenames), decodes supported DXT1/DXT3/DXT5 DDS files on the CPU, uploads RGBA textures to wgpu, and draws triangles in surface-specific batches. A checkerboard substitutes for missing or unsupported textures; messages are printed to stderr. Alpha blending is enabled for texture transparency, but sorting/cutout rules have not yet been implemented, so transparent geometry can render imperfectly. TGA and other formats are not yet enabled by the current image decoder configuration.

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Models\\Vehicles\\TEC\\asf.V3O"
```

UV direction and scale are still experimental pending comparison with the original EM4 editor. The source commit has not been independently compiled or GPU-tested.

## Viewer orientation and normal debug

The viewer now converts V3O positions and normals from an assumed EM4 Z-up axis layout into the Y-up rendering convention (`x, z, -y`). Test against several models to verify handedness and orientation. **W** switches between textured output and normal-vector visualization; **R** resets the camera. Existing orbit/pan/zoom controls remain unchanged. This is a normals diagnostic, not a true wireframe rasterization mode. Alpha blending is still basic and transparent meshes may display sorting artifacts.

## Viewer and prototype research update

The V3O viewer now uses directional plus ambient lighting and a flat ground reference just below the model. Press **G** to toggle the ground; **W** visualises normals; **R** resets the camera. Shadows and advanced transparency sorting are not implemented yet.

To inspect an E4P prototype without changing the source:

```powershell
cargo run -p openem4 --bin em4-e4p-inspect -- "C:\\path\\to\\vehicle.e4p"
```

This research CLI checks whether the file is readable text and lists candidate V3O/E4P path references. It does not yet parse binary prototypes, wheel placement, commands, or vehicle hierarchy. Do not assume it can reconstruct the full vehicle.

## Prototype-based vehicle assembly (experimental)

Use the existing viewer with an `.e4p` path:

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Prototypes\\Vehicles\\TEC\\asf.e4p"
```

Or inspect attachment references and positions as JSON:

```powershell
cargo run -p openem4 --bin em4-assembly -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Prototypes\\Vehicles\\TEC\\asf.e4p"
```

This is an experimental **first assembly pass**: reads the body `ModelName`, locates positioned wheel/door `Filename` records and their `PosX/PosY/PosZ/Rotation` fields, and resolves related child prototypes and V3O assets underneath the game `Data` directory. Geometry is combined in a shared mesh before rendering. Missing components are reported.

Limits: transform units and rotation conventions are provisional; child material texture directories may differ, and this stage does not implement door animation, wheel steering, lights, physics, nested children or a complete E4P parser. The original assets are not modified.

## E4P attachment diagnostics

The prototype viewer now prints every attachment's original coordinates and rotation matrix to the console. Child textures are loaded relative to each child V3O's directory, rather than the body-model directory.

Compare the default rotation convention against the experimental transposed convention:

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Prototypes\\Vehicles\\TEC\\asf.e4p" --transpose-attachment-rotation
```

The flag alters only the child attachment rotation, not the main vehicle geometry. It is a diagnostic comparison, not a confirmed Emergency 4 matrix convention. For unchanged transforms, omit the flag. Both paths read original game data without writing to it.

**Still pending:** GPU attachment-point gizmos, animation, full wheel rig, and verified local transform composition. Wrong wheel placements require screenshots/console diagnostics.
