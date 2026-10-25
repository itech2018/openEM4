# Original Emergency 4 format research

## Current inspector
`cargo run -p openem4 --bin em4-inspect -- "C:\\path\\to\\asset.dds"`

The inspector returns structured JSON without changing the original file.

- DDS: header magic, dimensions, mipmap count and FourCC extracted from bounded input.
- V3O: only distinguishes plausible readable ASCII from packed/unknown binary; makes no mesh-decoding claim.
- E4P and E4M: opaque identification; parsing not implemented yet.

The original Editor's `Modifications > unpack file` facility can produce an ASCII V3O. **Do not run destructive unpack operations on original game files**: make separate copies first.

Future: document independent V3O vertex/material grammar, test with owner-supplied nonredistributable fixtures, then add geometry decoding and a renderer bridge.

References:
- https://github.com/em4resource/EM4-Modding-Encyclopedia/blob/main/docs/editor-manual-reference.md
- https://oldversion.emergency-planet.com/tutorials/article/5-editing-your-unitxml-files/
