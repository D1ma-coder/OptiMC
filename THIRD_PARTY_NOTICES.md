# Third-party notices

The original OptiMC code is MIT licensed. Individual ports below retain their
upstream licenses; they are not relicensed as MIT. Runtime contains Rust/WGSL,
not a JVM or the downloaded mod JARs.

## Zoomify

Author: isXander. Source: https://github.com/isXander/Zoomify.
Reference binary: Zoomify 2.17.0+mc26.2, SHA-256
`485fb81d433002ec31eaa21720d6b7371fedbfb841e3f3bed3e8631dd80109d7`.
`src/mods/zoom.rs` is a modified Rust port of the interpolation/zoom behavior.
License: LGPL-3.0-or-later. Full LGPL and underlying GPL texts are in `LICENSES/`.
The corresponding Rust source and development oracle are included in this
source tree (`src/mods/zoom.rs`, `tools/reference/mods/ExportZoom.java`).

## Sodium

Authors: JellySquid and CaffeineMC. Source: https://github.com/CaffeineMC/sodium.
`src/mods/sodium.rs` ports the directional section visibility graph from Sodium
0.9.2+mc26.2; native terrain rendering integrates conservative graph traversal.
License: PolyForm Shield 1.0.0, reproduced in `LICENSES/Sodium.txt`.
This is a partial port, not Sodium's full rendering backend.

## 3d-Skin-Layers

Author: tr7zw. Source: https://github.com/tr7zw/3d-skin-layers.
`src/mods/skin_layers.rs` ports voxel wrapping, face-neighbor and corner rules
from 1.11.4; native avatar rendering integrates cached skin-layer geometry.
License: tr7zw Protective License, reproduced in `LICENSES/SkinLayers.txt`.
Player model placement and the complete set of upstream integrations remain
incomplete.

## ViaFabricPlus / ViaVersion

Sources: https://github.com/ViaVersion/ViaFabricPlus and
https://github.com/ViaVersion/ViaVersion.
`src/mods/via.rs` ports version-2 mapping codecs from the ViaVersion
5.12.2 snapshot embedded in ViaFabricPlus 5.0.3. Mapping data is embedded in
`data/mods/via-mappings.json.gz`. License: GPL-3.0-only; see `LICENSES/GPL-3.0.txt`.
Packet rewriting and old-server compatibility are not implemented by this port.

Exact versions, binary hashes, source links and development fixture hashes for
all four components are recorded in `data/mods/reference.json`. Development
oracles live in `tools/reference/mods/`; implementation and remaining scope are
documented in `docs/MOD-PORTS-2026-10-22.md`. Downloaded mod JARs are not bundled.
