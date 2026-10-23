# Changelog

## 0.1.0 — development snapshot

Initial public source publication, 23 October 2026. No stable binary release is declared.

- Independent Rust client targeting Minecraft Java 26.2 / protocol 776.
- Vulkan rendering through wgpu, official resource import and server chunk meshing.
- Multiplayer menus, local profiles, chat, HUD, inventory and player interactions.
- Partial entity, frame/map, sign and chest rendering.
- Configurable graphics, scene resolution, frame pacing and detail controls.
- CPU reference fixtures, local TCP tests and shader validation.
- Latest work: sign editor synchronization and heterogeneous NBT list handling; incoming light/weather state storage.

Full vanilla parity remains in progress. See [status](docs/STATUS.md), [coverage](docs/COVERAGE.md) and individual implementation notes for limitations and verification.
