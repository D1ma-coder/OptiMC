# Documentation

Start with the [English README](../README.md) or [Russian README](../README.ru.md).
Implementation notes below are primarily in Russian and record both verified behavior and remaining gaps.

## Project

- [Current status](STATUS.md)
- [Protocol sources](PROTOCOL.md), [audit](PROTOCOL-AUDIT.md), [coverage](COVERAGE.md)
- [App packaging](APP-PACKAGING.md)
- [GPU incident and testing constraints](GPU-INCIDENT-2026-10-20.md)

## Rendering and performance

- [Live world rendering](LIVE-WORLD-2026-10-21.md)
- [LOD](LOD-2026-10-22.md), [optimization](OPTIMIZATION-2026-10-22.md)
- [Shading](SHADING-2026-10-22.md), [presentation](PRESENTATION-2026-10-22.md)
- [Mod ports](MOD-PORTS-2026-10-22.md)
- [Maps and animations](MAPS-ANIMATION-2026-10-22.md)
- [Chests](CHESTS-2026-10-22.md), [entity text](ENTITY-TEXT-AUDIT-2026-10-21.md)

## Gameplay and synchronization

- [Inventory](INVENTORY-2026-10-22.md), [menu items](MENU-ITEMS-2026-10-22.md)
- [Block use](BLOCK-USE-2026-10-22.md), [entity picking](ENTITY-PICKING-2026-10-22.md)
- [Held items](HELD-ITEMS-2026-10-22.md), [mouse input](MOUSE-INPUT-2026-10-22.md)
- [World environment](WORLD-ENVIRONMENT-2026-10-23.md)
- [Loading lifecycle](LEVEL-LOADING-2026-10-23.md)
- [Light and weather](LIGHT-WEATHER-2026-10-23.md)
- [Sign editor](SIGN-EDITOR-2026-10-23.md)

## Layout

| Directory | Purpose |
|---|---|
| `src/assets/` | Resource import, models and reference data |
| `src/net/` | Protocol codecs, session state and synchronization |
| `src/world/` | World storage, meshing, picking and collisions |
| `src/render/` | Vulkan renderer, WGSL shaders, camera and interface |
| `src/mods/` | Individually licensed partial mod ports |
| `data/` | Embedded reports, mappings and test fixtures |
| `tests/` | Local TCP integration and shader tests |
| `tools/reference/` | Development-only Java reference exporters |
