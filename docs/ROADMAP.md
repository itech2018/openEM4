# Roadmap

- **v0.1:** Rust foundation, renderer diagnostic, filesystem and installation scanner.
- **v0.2:** Verified V3O model parsing, textures, E4P prototypes, model inspection.
- **v0.3:** E4M maps, terrain, scenery and world rendering.
- **v0.4:** Selectable units, vehicle controls, pathfinding and traffic.
- **v0.5:** Legacy scripting interpreter and EM4 API emulation.
- **v0.6:** Fire, medical, police and technical rescue simulation.
- **v0.7:** Mod compatibility tests and game behaviour parity.
- **v0.8:** Cooperative multiplayer and headless dedicated server.
- **v0.9:** Modern editor and optional enhanced rendering.
- **v1.0:** Playable release gated by tested compatibility.

Version milestones are goals and not fixed release commitments.

## Texture quality pipeline (agreed)

- **Now:** CPU-generated mipmaps and trilinear texture filtering in the V3O viewer; improve distant texture stability without modifying source DDS.
- **Next rendering pass:** classify opaque, cutout and blended materials from verified E4P/V3O evidence; render opaque before transparent, handle depth writes and sorting; inspect blue wheel material against the source DDS.
- **Implemented automatic viewer enhancement (v3):** conservative texture policy enabled by default; skips likely data maps, alpha-heavy sprites and large textures; chooses 2× or 4× Lanczos3 with 25% RGB sharpening and uses a versioned external content-addressed cache. `--texture-upscale=off` disables enhancement; `--texture-upscale=4` forces the previous experiment. No original asset modifications.
- **Constraints:** original assets stay untouched, no copyrighted enhanced textures redistributed, cache size and VRAM budgets enforced, asynchronous background generation does not block game simulation, reproducible fallbacks when a texture cannot be enhanced.

## Rendering upscalers — preferences

- **Preferred/current rendering:** native display resolution, with no frame upscaling or temporal reconstruction enabled by default.
- **Future options:** AMD FSR, NVIDIA DLSS and Intel XeSS as optional native renderer integrations after motion vectors, depth, history/jitter and compatible platform backends exist. No cloud AI API or per-frame paid API dependency.
- **Keep separate:** original-asset DDS texture processing is independent of finished-frame resolution upscaling.

## Automatic enhancement manager — staged work

- **Current:** synchronous on-load enhancement in the standalone V3O/E4P viewer; content-addressed external cache, basic filename/alpha/dimension heuristics, deterministic fallbacks. This is a first manager milestone, not full-game asynchronous streaming.
- **Next:** move the enhancement job to bounded background workers, prioritize currently visible textures, upload completed results to GPU on the render thread, and swap material bindings without blocking gameplay.
- **Later:** GPU/VRAM residency budgets, cache eviction policy, cancellation on mod changes, asset-specific classification (liveries, glass, terrain, normal maps), telemetry-free optional quality settings, cache invalidation and multi-mod stress tests.
- **Always:** source game and mod assets remain read-only; native display rendering remains preferred; no paid cloud AI dependency.
