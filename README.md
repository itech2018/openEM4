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

## Loading Emergency 4 mods

The prototype viewer supports both base-game `Data/Prototypes/...` and installed mods under `Mods/<mod name>/Prototypes/...`, for example:

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Mods\\London Mod v1.3\\Prototypes\\Vehicles\\Metropolitan Police\\astra01.e4p"
```

Relative `mod:` model and child-prototype references resolve against the selected mod root first, and fall back to the base `Data` directory when missing. Corona TGA lookup searches both, preferring the mod override. This fixes the previous fatal `E4P must be within an Emergency 4 Data directory` error. Some mods may use unsupported reference syntaxes or formats; the change has not been independently compiled on Windows.

## Mod vehicle material compatibility

The V3O/E4P viewer now resolves texture references relative to the source V3O first, and supports `mod:` texture paths via the selected mod's root followed by the base-game `Data` fallback. This also applies to vehicle child models (wheels and doors), whose texture paths can be absolute after E4P assembly. Missing textures remain visible as checkerboard fallback and report the missing reference in the console. Unused `Depth.texture` and `Viewer.index_count` warnings have been addressed; the depth texture is still retained for GPU resource lifetime. The changes are source-level and need verification with London Mod assets.

## Door motion preview (experimental)

Press **D** in `em4-view` to toggle a simple open/closed pose for child attachments classified as doors by the E4P assembly parser. The viewer keeps a copy of the closed vertex buffer, then rotates door-only vertices and normals around their decoded E4P child anchor. No source assets are modified. This is **not** yet authentic Emergency 4 door animation: hinge axis and 65-degree opening angle are provisional, child hierarchy/hinge definitions and multiple door controls remain to be reverse-engineered, and it does not update collision or gameplay. If no door child is decoded, D has no visual effect. The code has not been independently built or visually tested.

## Smooth individual door preview

Doors now interpolate smoothly between closed and a provisional 65° pose using eased, time-based motion (~0.77 seconds per full stroke). Press **0** to select all detected doors or **1–9** to select an individual door by its printed assembly order, then press **D** to toggle that selection. The viewer recomputes each frame from untouched closed vertices, avoiding cumulative drift, and redraws while doors move. The hinge and angle are provisional and do not yet decode native door animation. This is viewer-only and leaves original data intact.

## Door hinge preview correction

The previous door animation rotated children around their E4P attachment origin, causing implausible arcs and doors lifting away from the vehicle. The preview now estimates a vertical hinge at the front/rear edge of each door's **closed mesh bounds**, converts that hinge from EM4 Z-up coordinates into the viewer's Y-up system, and gives opposite sides opposite swing directions. The selected door controls (1–9, 0, D) and eased motion remain. Press **J** to print each inferred hinge, assigned swing and animation progress. This is an **inferred hinge**, not yet verified original E4P metadata; unusual door geometry or vehicle orientation may still require per-door overrides. Original assets are never edited.

## E4P door rotation axis and angle

The E4P assembly decoder now recognises `Door` records with `AxisX`, `AxisY`, `AxisZ` and `OpenAngle` immediately preceding the matching child `Filename`. The viewer uses the decoded axis (converted from Emergency 4 Z-up to viewer Y-up) and signed angle with Rodrigues rotation; where unavailable, the already tested geometry-inferred vertical swing and 65° angle remain the fallback. For example, the original ASF recovery platform has an E4P door entry with axis (0, 1, 0) and an open angle of -125°. The precise native hinge pivot and animation speed remain unverified; closed-geometry-based pivot inference still applies. The `D` and `0`–`9` selection controls remain unchanged. Run `cargo test -p em4-formats` to exercise the new decoding test. The changes have not been independently compiled.

## Ramp hinge and side-door regression correction

The E4P viewer now distinguishes **central horizontal-axis equipment** (such as the ASF's recovery ramp) from **off-centre vehicle side doors**. For horizontal equipment near the body's lateral centreline, it uses the E4P child's placement origin as the provisional rotation pivot and the native rotation axis and angle. Ordinary side doors instead keep the previously user-verified geometry hinge and ±65° swing, preventing the London Mod Astra regression introduced when native axes were applied to every door. The classification uses attachment position and body width and remains a heuristic; print `J` diagnostics and visually validate each vehicle. This does not yet decode an explicit native hinge point.

## Mod side-door regression safeguard

After native E4P axes were enabled, the London Mod Astra could rotate door parts across its windscreen. The viewer now **defaults to the last verified geometry-based side-door animation for installed mods**. The base-game ASF keeps its native horizontal ramp mode. To experiment with native equipment axes in a mod, pass `--native-mod-door-axes` after the E4P filename; this flag is for diagnostics and not the recommended default. The viewer prints the chosen pivot, axis and mode. Actual per-prototype door hinge metadata still requires research; avoid applying generic native-axis assumptions to all modded car doors.

## Optional 4× spatial texture upscaling (first implementation)

The V3O/E4P viewer now supports `--texture-upscale=4`, using CPU Catmull–Rom interpolation for decoded material textures, with a BLAKE3 content-addressed PNG cache. Cached results are stored in `%LOCALAPPDATA%\\openEM4\\textures\\spatial-v1` on Windows (or `$XDG_CACHE_HOME/openEM4/textures/spatial-v1` / `~/.cache/openEM4/textures/spatial-v1` on Linux). Delete the directory at any time to regenerate. Original DDS/TGA assets are read-only. Upscaling is disabled by default; this first implementation generates textures on load, not asynchronously, and **does not provide AI super-resolution or new source detail**. Output is limited to 8192 pixels per side and 64 MiB decoded RGBA per texture to control memory usage; larger textures fall back to original resolution. Corona TGA sprites are excluded for now. 

```powershell
cargo run -p openem4 --bin em4-view -- "C:\\Program Files (x86)\\Steam\\steamapps\\common\\EMERGENCY 4 Deluxe\\Mods\\London Mod v1.3\\Prototypes\\Vehicles\\Metropolitan Police\\astra01.e4p" --texture-upscale=4
```

The changes have not been independently compiled in the development environment.

## 4× Lanczos3 and 25% sharpening (October 2026)

Use `--texture-upscale=4` in the V3O/E4P viewer for 4× Lanczos3 enlargement followed by a mild 25% RGB unsharp mask. Alpha is resized but **not sharpened**. This replaces the earlier Catmull–Rom result that softened vehicle lettering and liveries. The resulting PNG cache is versioned separately as `%LOCALAPPDATA%\\openEM4\\textures\\lanczos3-sharp25-v2` on Windows (or under the Linux cache directory); older `spatial-v1` cache files will **not** be reused. Original DDS/TGA assets are never overwritten. The default remains original-size textures, and the chosen framebuffer rendering mode is **native resolution**, not DLSS/FSR/XeSS. Those rendering upscalers are future options, distinct from asset-texture upscaling. The amount is presently fixed at 25%, not a runtime slider. Compare the same camera framing with and without the flag before assessing texture clarity.

Verification: `cargo test -p openem4 --bin em4-view`, then launch your Astra prototype with and without `--texture-upscale=4`. Compilation and image results have not yet been independently verified.

## Automatic texture enhancement for installed mods (preview)

The V3O/E4P viewer now **automatically** checks each loaded material texture, including London Mod models, and chooses whether to enlarge it. No per-mod command is needed. The conservative current rules skip likely normal/specular/roughness maps, corona images, alpha-heavy textures and already-large images; smaller eligible textures use 2× or 4× Lanczos3 plus 25% RGB-only sharpening. Cached PNG results are stored outside Emergency 4 at `%LOCALAPPDATA%\\openEM4\\textures\\automatic-v3` on Windows. Original DDS/TGA files are unchanged.

Launch normally for automatic enhancement, pass `--texture-upscale=off` to disable, or `--texture-upscale=4` to force the older 4× comparison mode. The cache version changed to avoid reusing stale textures. **Current implementation runs synchronously while a model loads**; it is not yet a background streaming manager and may increase initial load times. Heuristic classification cannot identify every asset correctly. Future full-game work includes asynchronous workers, GPU material replacement, VRAM budgeting, and quality-aware data-map handling. Native output rendering stays the default; DLSS/FSR/XeSS remain separate future choices.

## Non-blocking automatic texture enhancement (prototype)

The viewer now renders the original DDS materials as soon as they are decoded, rather than waiting for optional enlargement. Two background worker threads process eligible 2×/4× Lanczos3 enhancement jobs, with a queue limited to 16 pending textures. At most one completed replacement is uploaded to the GPU during each event-loop iteration, and the existing surface material binding is replaced without changing vehicle geometry, doors or lighting. If the queue is full, the original material stays in use. The existing external content-addressed cache is reused. Console progress reports queued jobs and completed GPU replacements. This is not yet full scene streaming, visibility-prioritized enhancement or a strict GPU/VRAM budget; no native rendering upscale is used. Compilation and the visual result still require local verification.

## Background enhancement deduplication

The viewer now groups identical enhancement inputs (decoded RGBA bytes, dimensions and requested upscale factor) into a single background job. Once that job completes, all matching surfaces reuse the resulting GPU material bind group. The grouping records surface indices until the render thread receives the finished texture, avoiding races when the worker finishes before model loading does. The 2-worker/16-pending-job limits remain, originals render immediately, and source files stay untouched. Console output reports queued unique jobs, duplicate requests and upgraded surface counts. More advanced scene visibility priority, VRAM budgeting and persistent cross-session GPU residency are still pending. This source change has not been independently compiled or visually tested.

## Enhanced-texture GPU budget (preview)

The standalone viewer now limits newly uploaded **enhanced** texture resources to an estimated 256 MiB per model session, counting all generated mip levels. When the cap is reached, completed enhancement results are discarded and original textures remain visible. Duplicate surfaces share one enhanced allocation. This is a conservative incremental upgrade allowance, **not** an accurate total VRAM measurement: the baseline textures, geometry, renderer buffers and temporary in-flight copies still consume GPU memory. Runtime adaptive budgeting, visibility priority, eviction and total GPU residency tracking are subsequent milestones. This update does not alter asset files or vehicle animation behavior, and has not been independently compiled.

## Enhancement upload priority (initial)

Instead of submitting texture enhancement jobs immediately in arbitrary V3O surface order, the viewer now stages unique candidates, ranks them by associated mesh triangle count, and sends higher-coverage candidates to the bounded worker queue first. Original textures still render immediately, duplicate content is processed once, and the 256 MiB incremental enhanced-texture GPU allowance remains. This is a **static mesh-complexity proxy**, not actual camera visibility or projected pixel coverage. Later full-game work must add dynamic visible-object priority, cancellation/reprioritization, streaming residency and eviction. No vehicle animation code is changed.

## Initial camera-aware texture enhancement priority

The viewer now ranks pending enhanced-texture jobs using a combination of each material's triangle count, the distance from the opening camera to its surface centre, and whether that centre lies in the camera's forward hemisphere. Shared texture jobs use the highest score among their associated surfaces. This improves first-load prioritisation compared with file order or triangle count alone; it **does not** yet re-sort pending work when the camera moves, account for true occlusion, or calculate exact screen-pixel coverage. The existing two workers, bounded queue, duplicate detection, 256 MiB incremental GPU enhancement budget, and original-texture fallback are unchanged. Full dynamic visibility scheduling remains a future milestone.

## Dynamic camera-prioritized enhancement (viewer preview)

Pending texture enhancement candidates now remain available after model loading instead of being truncated at the first 16 entries. At most two enhancement jobs are in flight. Each time a worker finishes, the viewer recalculates the priority of remaining textures using the **current** orbit/pan/zoom camera position, surface-centre direction, distance and triangle count. The completed original-texture fallback and 256 MiB enhanced GPU allocation allowance remain intact. In-flight jobs are not pre-empted, and this is still a surface-centre heuristic rather than true occlusion or projected pixel coverage. `--texture-upscale=off` continues to disable enhancement. Compile and visual verification required.

## Enhanced GPU texture residency and eviction (preview)

The V3O viewer now retains original GPU material bindings for every surface and tracks which enhanced texture groups are resident within the existing 256 MiB additional-enhancement allowance. If a completed texture would exceed that allowance, the viewer chooses the least relevant enhanced resident based on current camera facing and distance, restores its original material binding, and accounts for the estimated released mipmapped allocation before attempting the new upgrade. Identical surfaces continue to share an enhanced material. This is an **approximate enhancement-only GPU budget**: actual driver memory residency, GPU execution lifetime and total scene VRAM aren't measured, and evicted assets are not yet automatically requeued. Compilation and runtime validation remain necessary.

## Evicted enhancement restoration (streaming preview)

When an enhanced material is evicted due to the 256 MiB incremental GPU allowance, openEM4 retains its original material and remembers the enhancement job. After a 5-second cooldown, an evicted texture can be queued again **if its current camera relevance exceeds the least relevant resident by at least 25%**. Requeued work uses the same content-addressed on-disk cache to avoid recalculating textures when cached. A restored texture replaces its original material on the rendering event loop; the two-worker limit and fallback behaviour remain. This is a basic eviction/restoration feedback loop; exact GPU VRAM usage, sustained-budget fairness, LRU history and true projected-screen-area priority are not implemented yet. This patch has not been independently compiled on Windows.

## Cache management and adapter-sensitive budget

On viewer launch (with enhancement enabled), openEM4 scans only its generated `automatic-v3` texture cache and prunes `.png` files when it exceeds **2 GiB**, oldest modified first, targeting 90% of that capacity. Original `Data/` and `Mods/` trees are not traversed. A prune failure is nonfatal. This is a *cache-size* limit, not a free-disk-space monitor, and access does not currently update file recency. Enhanced GPU allowance is chosen conservatively from the wgpu adapter class: discrete 256 MiB, integrated 96 MiB, virtual/CPU 64 MiB, other 128 MiB. These are heuristic limits, not actual VRAM detection. The viewer retains original materials and logs the chosen budget. Runtime verification on Windows remains required.

## Texture residency accounting and priority guard

The viewer now totals the estimated mipmapped RGBA memory of all baseline material textures and reports this separately from the resident enhanced-texture allowance. An enhancement that would exceed its allowance will not evict a more camera-relevant resident merely because its worker job finished first: a 25% importance margin is required before displacement. Deferred enhancements are flagged for later restoration through the existing cooldown and on-disk cache. These are **per-viewer estimates**, not driver-reported VRAM, and do not yet aggregate textures across a game-wide scene manager; mesh buffers, render targets, transient GPU allocations and texture deduplication of original assets are not fully counted. The current code is pushed but has not been independently built or tested on Windows.

## Shared GPU material foundation

Original vehicle textures are now deduplicated by decoded RGBA content, dimensions and sRGB interpretation, not by mod filename. Surfaces with identical texture content share one `Arc<wgpu::BindGroup>` and one underlying GPU upload instead of allocating identical baseline resources repeatedly. Base texture memory estimates now count unique uploads, with reuse diagnostics logged at load. The original materials remain retained for restoration after enhanced texture eviction. This establishes a viewer-level content-addressed resource registry; a true **game-wide** registry shared between simultaneously loaded objects, render passes and maps is still to be implemented as the scene architecture grows. Compilation and runtime testing on Windows remain outstanding.

## Reusable GPU material registry (engine groundwork)

The viewer now uses `apps/openem4/src/material_registry.rs`, a reusable content-addressed resource registry rather than maintaining its own GPU upload lookup table. The registry deduplicates original RGBA8-sRGB texture materials by dimensions and decoded content, returns shared `Arc<wgpu::BindGroup>` handles, and tracks estimated unique mipmapped texture bytes. The V3O/E4P viewer is its first consumer. This is a foundational reusable module, **not yet a single live global registry across all scene objects**: full-map renderers will need to share one registry tied to the same GPU device, sampler and bind-group layout. Enhanced-texture streaming, residency, and animation behavior are otherwise unchanged. Validate using `cargo test -p openem4 --bin em4-view` and your regular London Mod viewer command; compilation has not been independently verified in this environment.

## Multi-object V3O scene preview (experimental)

The viewer now supports `--instances=N` for **standalone V3O models**, with N from 1 to 16. It arranges N instances of the same parsed mesh on a grid in native EM4 coordinates, then renders the combined scene using a single set of material surfaces and the existing content-addressed material registry. This is a tangible multi-object preview for shared GPU resource handling; it does not yet import arbitrary heterogeneous scene objects or E4M world maps.

Example: `cargo run -p openem4 --bin em4-view -- "C:\\path\\to\\model.v3o" --instances=4`

E4P prototypes continue operating as before with doors, ASF attachments and emergency lights. Instance preview is deliberately rejected for E4P to prevent duplicating attached parts without corresponding animation metadata. Scene assembly is bounded to 16 instances and 2 million vertices/triangles. The implementation has not been independently compiled on Windows.

## Mixed-model V3O scene preview

To inspect multiple **different** V3O meshes together, pass additional models with `--add-v3o=<full path>`. Example (PowerShell):

```powershell
cargo run -p openem4 --bin em4-view -- "C:\path\to\vehicle.v3o" "--add-v3o=C:\path\to\building.v3o"
```

The preview supports up to 16 independently loaded standalone V3O models, places them side by side, remaps vertex indices and surface indices, and resolves each model's texture references against its own original path. The renderer's shared material registry then reuses GPU resources when different models contain identical decoded textures. `--instances=N` remains available for repeated copies of *one* V3O and cannot currently be combined with `--add-v3o`. E4P vehicles cannot yet be merged into mixed scenes because doors, attachments and lighting need independent object-level state. The feature is a combined-mesh **preview**, not yet independent per-object transforms, physics or game-world rendering. Windows compilation and visual confirmation remain outstanding.

## Scene entity foundations

The mixed-V3O preview now constructs individually identified scene records through `apps/openem4/src/scene_entities.rs`. Each `SceneEntity` holds a stable session ID, source name and a validated `Transform` with EM4 Z-up translation, yaw and scale. `scene_mesh::combine_entities` applies each model's position and normal transformations before assembling the preview geometry, and rejects duplicate IDs or invalid transforms. In the current command-line preview all new transforms start at identity and objects are automatically placed in a row by the existing scene assembler. Scene IDs are logged when `--add-v3o` loads additional models.

**Limit:** transforms are currently baked into combined preview geometry. Runtime per-object editing/movement, entity-component lifecycle, separate draw calls, object picking, E4P vehicle entity animation and E4M map loading are subsequent milestones. Original model assets, shared texture registry and existing E4P vehicle animations are unchanged. Verify locally with `cargo test -p openem4 --bin em4-view`; this commit has not been independently compiled.

## Runtime mixed-scene movement (preview)

In a mixed-V3O scene opened with one or more `--add-v3o=...` arguments, `Tab` selects the next loaded scene object. Use `Left/Right` to move it sideways, `Up/Down` to move it in depth, and `Page Up/Page Down` to move it vertically. Each press moves the selected object by 0.5 renderer units. The viewer updates just that entity's vertex-buffer range with `wgpu::Queue::write_buffer`; it does **not** rebuild the whole combined mesh or reupload unaffected geometry. Original source data remains unchanged; the source model's original vertices are retained for deterministic cumulative offsets.

This is **runtime translation** using per-object vertex-buffer ranges, *not yet full shader-driven independent model matrices or independent GPU draw objects*. The preview deliberately enables it only for heterogeneous V3O scenes, leaving E4P animated door/ramp/light handling unchanged. Current camera-distance texture priority still uses load-time surface centres and needs per-entity offset updates later. Compilation and interactive testing remain to be verified.

## Mixed-scene blank view diagnostic

Mixed V3O models can have much larger geometry extents than a standalone vehicle. The viewer now adjusts its far clipping plane from the current camera distance rather than using a fixed projection depth, and prints world-space scene bounds and camera distance. This targets the blank grey viewport seen when previewing London Mod `astra01.v3o` with `Barclays.v3o`; it does not yet verify that the particular source meshes have reasonable bounds. If the preview remains blank, share the `Scene bounds` log line and the renderer diagnostic lines so model coordinate scales can be inspected. This patch has not been independently run with those London Mod assets.

## Mixed-scene runtime rotation

With multiple V3O files loaded using `--add-v3o`, select an entity with **Tab**, translate it with the arrow/Page keys, and use **Q** (rotate -15°) or **T** (rotate +15°) to rotate the selected object around its initial bounding-box centre. Positions and normals are transformed from retained initial geometry on every edit to prevent cumulative drift; only that entity's vertex-buffer range is updated. This is a preview-side vertex update, **not yet per-object shader transform matrices**. The E4P animated vehicle path is unchanged. Compilation and interactive validation are still required.

## Per-entity GPU model matrix preview

Mixed V3O scenes now use a dedicated object-transform uniform bind group (`@group(2)`) in the vertex shader. Each scene entity receives its own model matrix, with translation and yaw about the object's initial XZ bounding-box centre. Press Tab to select an entity, arrow/Page keys to translate, or Q/T to rotate it. Only the selected entity's 64-byte uniform is written when editing, not its whole vertex range; opaque, cutout and transparent surface passes bind the corresponding object's transform. The ground plane and E4P diagnostic light markers use the identity transform, preserving their established placement. The original vehicle E4P door and ramp vertex animations remain on their existing path.

This is a **prototype GPU transform** for combined geometry, not a fully independent draw-object renderer. Current camera-distance/texture priorities and transparent sorting still refer to initial surface centres, and scaling/parenting/object lifecycle are later work. Validate on Windows with `cargo test -p openem4 --bin em4-view` and the working Astra + Barclays `--add-v3o` command; this change has not been independently compiled here.
