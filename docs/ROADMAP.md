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
- **Confirmed default future enhancement:** 4× **spatial** texture upscaling (user-selectable Off / 2× / 4×). Preserve original assets byte-for-byte. Cache generated textures outside game directories, keyed by source-content hash, scaler version and settings; keep alpha and edges intact. Cache can be deleted safely, enforce disk/VRAM budgets and use unenhanced textures during processing. Spatial enhancement is preferred; optional AI methods are out of scope for this milestone.
- **Constraints:** original assets stay untouched, no copyrighted enhanced textures redistributed, cache size and VRAM budgets enforced, asynchronous background generation does not block game simulation, reproducible fallbacks when a texture cannot be enhanced.
