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

## ASF wheel-placement scale correction

The original ASF E4P wheel positions use offsets approximately X=+98/+96 for one axle and X=-58/-56 for the other (Y around +/-40, Z around 21). The previous viewer erroneously divided E4P attachment translations by 100 while V3O vertex coordinates were already converted into the viewer's working units. The assembler now preserves the E4P offsets directly. This is based on the ASF model and needs visual confirmation with other prototypes. The `--transpose-attachment-rotation` comparison option remains available for rotation diagnostics.

## Texture filtering improvements

The V3O viewer now builds a complete CPU mipmap pyramid from decoded vehicle textures and enables trilinear filtering. These mipmaps are in GPU memory only; original game DDS files are never changed. Future automatic 2×/4× texture enhancement (including optional AI super-resolution) is in `docs/ROADMAP.md` and **is not yet implemented**.

Rebuild and open the usual `asf.e4p` with `em4-view` to compare distant body/wheel texture aliasing.

## Opaque and transparent material passes

The model viewer now classifies a material as transparent when any decoded texture pixel has alpha below 250. It renders fully opaque surfaces first with depth writes, then draws transparent surfaces with alpha blending and depth writes disabled. This is an initial material heuristic, **not** a verified EM4 surface-mode implementation. Transparency is not yet distance-sorted, and cutout geometry may require alpha-test treatment; visually incorrect windows or wheels should be reported.

**Confirmed future texture enhancement:** 4× spatial upscaling, optional/off switch, and a derived-texture cache outside Emergency 4's installation. The original textures will remain unchanged. The enhancement pipeline and cache are planned, not yet implemented.

## Material rendering refinement

The V3O/E4P viewer now uses three material paths based on decoded DDS alpha:
- **Opaque:** alpha >= 250 everywhere, normal depth writes.
- **Cutout:** alpha values clustered near zero or full coverage; shader discards texels below 0.5 and writes depth for surviving pixels.
- **Blended:** intermediate alpha; depth tests enabled but depth writes disabled, with surface batches ordered back-to-front by approximate camera distance.

Each surface classification is printed at startup to help diagnose wheels or windows. The heuristic does not yet decode EM4-specific material flags, and sorting by surface centre is approximate (overlapping triangles inside one surface can still render incorrectly). Wheel coloration could also be a missing/incorrect child texture, so check console messages before treating it as an alpha problem.

**Test:** `cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Data\\Prototypes\\Vehicles\\TEC\\asf.e4p"`

This update has not been independently compiled or visually validated.

## Lighting and material diagnostics

Vehicle materials now use a directional key light, subtle opposite-side fill and hemispherical ambient illumination. The viewer also logs per-surface texture path, dimensions, material class and minimum/maximum alpha values. Use those logs to diagnose missing wheel textures or unexpectedly transparent materials. This change does not modify original DDS assets. It is not physically based rendering, and original Emergency 4 material flags still require decoding. Build and appearance must be verified locally.

## Experimental vehicle light controls

The V3O/E4P viewer now has a **light state and timing foundation**: H toggles headlights, B toggles brake lights, L and K toggle left/right indicators, and E toggles emergency beacon state. A timer computes indicator and beacon blink phases, and the viewer redraws while timed states are active. These are diagnostic control states only: **no light meshes, emissive surface selection, beams, colored flashing points, or authentic E4P lighting data have been implemented yet**. Console messages confirm toggles. Vehicle geometry and its original textures remain unchanged.

## Visible vehicle-light preview

The V3O viewer now draws **temporary colored diagnostic quads** when vehicle light states are enabled: `E` alternates red/blue markers, `H` shows two white headlight previews, `B` shows rear red brake previews, and `L`/`K` flash amber turn markers. Positions are estimated from the assembled mesh's bounding box, **not parsed from E4P light definitions**. These are unlit/glow-color preview geometry, not actual bloom, point lights or headlight projections, and might be obscured by body geometry. The original mesh and DDS files are unchanged. The marker positions must be replaced with real prototype-driven emitters. The code has not been independently built or visually verified.

## E4P original vehicle light definitions (experimental)

The viewer now reads light point sets directly from the original vehicle `.e4p`. The binary decoder also extracts each light's `coronatex`, `flashfreq`, `flashoffs`, `coronasize`, `lighttype` and `color`. Approximate bounding-box markers are removed. Markers are placed at the mean of each decoded light's point set, converted from EM4 Z-up coordinates to viewer Y-up. Console output lists every recovered light.

Important limitations: this is the first experimental decoder for the observed ASF binary E4P layout, **not a verified complete light definition interpreter**. The markers are still plain coloured quads, not actual textured coronas or lights. The type-to-control mapping uses corona texture filenames as a heuristic; full brake/indicator/headlight categorisation, source colour, polygon orientation, intensity and original game flashing semantics need validation. No original assets are modified. Testing must confirm E4P records decode correctly in the Windows build.

## Source-defined E4P light polygons (ASF correction)

The original ASF file contains 17 binary `light` records with point lists. Some represent a polygon spanning multiple positions, **not** a single emitter centred at the arithmetic mean. The preview now triangulates each recorded list (or draws a small marker for a single point), preserving the recorded coordinates. The prior centroid-based preview could put a floating light in the wrong location.

This is a diagnostic geometry pass. It still does not reproduce the engine's original light texture mapping, corona billboarding, per-light colour/intensity, material flags or complete flash modes. Some recorded polygons can be coplanar with vehicle geometry and hidden by the depth test; source point data should be inspected in console output before assuming a missing light.

## Light-point diagnostic correction

The E4P viewer now shows a small separate marker at **every** parsed light point instead of connecting points into filled red/blue polygons. The light parser also uses a shorter bounded scan and stops at additional prototype section keys to reduce accidentally combining unrelated records. This remains a diagnostic, not confirmed original-game corona rendering; marker positions need validation against source files. The repository update has not been locally compiled.

## E4P light anchor correction

The ASF binary E4P stores a separate `offset` vector (UTF-16 string such as `163, -47, 30`) in each light record. Previous diagnostics incorrectly used `points` as model-space locations, producing distant floating markers. The decoder now extracts the offset into `Light.position` and the viewer renders one marker at that anchor, preserving local polygon points for later corona/area-light geometry decoding. Light entries with no valid offset are not rendered. Full E4P hierarchy transforms, corona visuals and behaviour are not yet validated. Original assets remain read-only.

## Camera-facing, scaled corona previews

The E4P light preview now uses each decoded `offset` anchor with the corresponding `coronasize` (bounded for safety), rather than a fixed-size square. Preview coronas rotate to face the camera and use a soft radial alpha gradient and alpha blending. The existing E4P flash frequency/offset logic and E/H/B/L/K controls remain. This is **procedural corona rendering**, not yet sampling the game's actual `coronatex` TGA texture. Real corona texture loading, light-type mapping, correct per-light orientation and bloom remain future work. Original Emergency 4 files are never modified.

## Original Emergency 4 corona texture sampling

The viewer now looks up each E4P `coronatex` file (including legacy `.tga` files) under the original game's `Data` directory, decodes it with the Image TGA loader, and binds a separate alpha-blended sprite material for each light. Coronas retain their decoded E4P `offset`, `coronasize`, flash frequency and flash offset, and face the camera. Missing/unsupported textures use an in-memory soft radial fallback and report a warning. The original files are never altered. Light category controls remain heuristic, real bloom and precise EM4 light-type semantics are not implemented, and the game Data tree lookup is presently bounded but may be slow on large mod installations. The changes have not been independently compiled or visually tested.

## Corona visibility regression fix

After original TGA support was introduced, emergency-light sprites could disappear because some legacy images have unusable alpha channels or light quads fail the depth test. The viewer now recovers alpha from RGB brightness **only** when the decoded alpha is uniform, applies an additional soft radial falloff, and renders light billboards without depth rejection so source-defined offsets remain visible. This is a deliberate temporary preview trade-off: a light may appear through solid vehicle geometry. Proper occlusion will require source-specific light semantics. Console logs report original TGA loads or procedural fallbacks. This commit does **not** yet include accurate original flash-pattern interpretation, bloom, or a persistent on-disk corona cache. Original data remains unchanged.

## Corona depth comparison and texture lookup reuse

Press **O** in `em4-view` to toggle corona depth occlusion. Default remains **off**, matching the previously visible emergency light previews. With occlusion on, light sprites depth-test against the truck and may disappear if an E4P offset is inside geometry; turn it off to diagnose. Original `coronatex` filename lookups are now reused per prototype by case-insensitive texture name to avoid repeatedly scanning the Data tree for the same file. This is an in-process lookup cache, **not** the future persistent upscaled texture cache. Full EM4 flash patterns, lighting types and bloom remain pending.

## Per-light E4P flash phase — initial behaviour pass

Each light now evaluates its own recorded `flashfreq` and `flashoffs`, using a reusable, tested flash-phase helper. Red/blue corona lights are controlled by the emergency beacon key `E`; the `B` brake key no longer wrongly activates all red emergency beacons. Headlights and amber indicators retain their preview controls. The visual flash interpretation is still **provisional**: the original E4P light-type enum and flash frequency units are not yet fully verified, and categorisation still uses corona filenames. A follow-up pass must decode explicit light types, duration/duty cycle, vehicle-specific groups, and indicator left/right membership without guessing from colours. The update has not yet been independently compiled on Windows.
