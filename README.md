<h1 align="center">
  <img src="docs/assets/optimc-wordmark.svg" alt="OptiMC" width="900">
</h1>

**English** · [Русский](README.ru.md)

[![CI](https://github.com/D1ma-coder/OptiMC/actions/workflows/ci.yml/badge.svg)](https://github.com/D1ma-coder/OptiMC/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.88%2B-orange?logo=rust)](Cargo.toml)
[![Minecraft](https://img.shields.io/badge/Minecraft-26.2-65b447)](docs/PROTOCOL.md)
[![Status](https://img.shields.io/badge/status-experimental-yellow)](docs/STATUS.md)
[![Licenses](https://img.shields.io/badge/licenses-MIT_%2B_upstream-blue)](THIRD_PARTY_NOTICES.md)

OptiMC is an independent Minecraft Java 26.2 client built in **Rust**, with **Vulkan rendering through wgpu**. It implements its own networking, world storage, meshing, interface and player simulation. The application does not run Minecraft Java code or embed a JVM.

The focus is multiplayer, responsive rendering and configurable detail. Official resources are downloaded or imported locally. There is no integrated server or singleplayer mode.

**Experimental software.** Server worlds, inventories and interactions are implemented, but full vanilla compatibility is still in progress. Some packets and entity renderers are missing or partial. See the [protocol coverage](docs/COVERAGE.md) and [development status](docs/STATUS.md) before relying on a particular feature.

**[Build](#building-from-source) · [First connection](#first-connection) · [Controls](#controls) · [Graphics](#graphics-and-performance) · [Documentation](#documentation)**

## Features

| Area | Current implementation |
|---|---|
| Multiplayer | Offline-mode login, configuration registries, compression, keepalive, teleport handling and world transitions; targets protocol 776 |
| World | Server chunks, block updates, official block models, incremental meshing and shared world snapshots |
| Player | Movement, collisions, first/third-person cameras, camera smoothing, attack/use and block interactions |
| Interface | Main menu, server list, local profiles, chat and command suggestions, hotbar, inventory, HUD, Tab and scoreboard |
| Special rendering | Item frames and maps, signs and sign editor, chests and selected entities; coverage remains partial |
| Graphics | Low/Medium/High presets, shadows and post-processing, GUI scale, render resolution, FPS cap and presentation settings |
| Optimization | Background meshing, mesh reuse, culling and adjustable detail; procedural terrain also has a quadtree LOD demo |

Local profiles can resolve Ely.by skins **without Ely.by login**. Microsoft account authentication and Realms are not implemented. Older-server translation is not implemented; the ViaVersion-related port currently covers mapping codecs only.

## Building from source

Install **Rust 1.88+**, a native compiler and a Vulkan-capable graphics driver. Use stable Rust for development.

```sh
git clone https://github.com/D1ma-coder/OptiMC.git
cd OptiMC
cargo build --release --locked
cargo run --release --locked -- assets sync
cargo run --release --locked
```

`assets sync` obtains official version metadata and resources from Mojang and verifies the downloaded JAR against its SHA-1. To import a locally installed client instead:

```sh
cargo run --release --locked -- assets import /path/to/26.2.jar
```

Resources, profiles and settings live in the ignored `.cache/` directory. Set `OPTIMC_CACHE` or use `--cache` to select another directory. Minecraft JARs, textures, downloaded mod binaries and personal profiles are not distributed in this repository. `data/` contains reference reports, protocol fixtures and mapping data used by the implementation and tests; see its provenance documents.

### Platforms

| Platform | Graphics runtime | Validation |
|---|---|---|
| macOS Apple Silicon / Intel | Vulkan via MoltenVK | Development host: Apple M2 Pro; current checks are CPU/TCP only |
| Windows | Vulkan loader and GPU driver | Build/check job configured in CI; hardware testing pending |
| Linux | Vulkan loader and GPU driver; X11 or Wayland | Build/check job configured in CI; hardware testing pending |

On macOS, install the [Vulkan SDK](https://vulkan.lunarg.com/sdk/home#mac), or place MoltenVK at `.cache/runtime/lib/libMoltenVK.dylib` with `libvulkan.dylib` pointing to it. See [app packaging](docs/APP-PACKAGING.md). GPU experiments previously caused a macOS WindowServer failure on the development machine; the [incident report](docs/GPU-INCIDENT-2026-10-20.md) records the limits of current graphics validation.

## First connection

Launch the application and choose **Multiplayer**, or connect directly:

```sh
cargo run --release --locked -- status localhost:25565
cargo run --release --locked -- play localhost:25565 --username OptiPlayer
```

The client currently supports Java servers using `online-mode=false`. Server-side authentication plugins may require additional steps in chat. Compatibility with anti-cheat plugins is not guaranteed.

For a diagnostic session without a game window:

```sh
cargo run --release --locked -- connect localhost:25565 --username OptiPlayer --seconds 30
```

## Controls

Default controls use physical keys where applicable, including T/E on non-English layouts.

| Input | Action |
|---|---|
| WASD, Space, Shift, Ctrl | Move, jump, sneak, sprint |
| Mouse / left click / right click | Look / attack or mine / use or interact |
| E | Inventory |
| 1–9 / mouse wheel / F | Hotbar / scroll slots / swap hands |
| T / slash | Chat / chat with `/` inserted |
| Tab | Player list; chat completion while editing |
| F5 / F1 / F3 / F11 | Camera view / hide HUD / debug / fullscreen |
| Esc | Close interface or open pause menu |

Inventory handling includes left/right click, Shift transfer and other click modes. Prediction and specialized container rules are still incomplete; the server remains authoritative.

## Graphics and performance

Graphics settings expose GUI scaling, proportional scene resolution from HD to screen resolution, an FPS cap, presentation mode, quality presets and a detail control.

- **Low:** classic textures and simpler shading.
- **Medium / High:** native WGSL lighting and post-processing inspired by BSL/Bliss. These are not complete ports of those shader packs.
- **VSync off:** prefers Mailbox, with FIFO fallback when Mailbox is unavailable. Immediate presentation is avoided to prevent tearing; FIFO fallback can still cap presentation at the display refresh rate.
- **LOD:** adjustable detail and shader/geometry budgets. A persistent distant-server-world LOD cache is not implemented; the client cannot render chunks the server has never supplied.

Rendering work runs separately from network processing. Shared snapshots, cached CPU/GPU meshes, bounded queues and staged uploads reduce redundant work. Performance varies by scene, quality and hardware; no general FPS improvement is claimed from isolated CPU benchmarks.

The procedural terrain laboratory is available separately:

```sh
cargo run --release --locked -- demo --radius 1024
cargo run --release --locked -- bench --radius 1024
```

## Development and checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Default tests include CPU logic, shader validation and local TCP fixtures. Asset-dependent and GPU tests are ignored by default. Do not use `--include-ignored` as a routine check. Java helpers under `tools/reference/` exercise downloaded reference classes during development only; Java is not an application runtime dependency.

Known gaps include recipes, advancements, passengers, equipment and parts of entity metadata/animation. Received light arrays and weather values are stored, but are not yet fully applied to world rendering. Respawn semantics, full font behavior and specialized inventories also need further work. The [coverage report](docs/COVERAGE.md) distinguishes partial handling from missing functionality.

## Documentation

- [Documentation index](docs/README.md): architecture and focused implementation notes.
- [Development status](docs/STATUS.md): checkpoints, verification and remaining work.
- [Protocol](docs/PROTOCOL.md) and [coverage](docs/COVERAGE.md).
- [Mod ports and limitations](docs/MOD-PORTS-2026-10-22.md).
- [Contributing](CONTRIBUTING.md) and [changelog](CHANGELOG.md).

## Star History

[![Star History](https://api.star-history.com/svg?repos=D1ma-coder/OptiMC&type=Date)](https://www.star-history.com/#D1ma-coder/OptiMC&Date)

## License

Original OptiMC code is [MIT licensed](LICENSE). Individual ports retain LGPL, GPL, PolyForm Shield and tr7zw Protective terms; see [third-party notices](THIRD_PARTY_NOTICES.md) and [LICENSES](LICENSES). The complete source tree must not be treated as MIT-only.

Minecraft resources belong to Mojang/Microsoft and are obtained separately. OptiMC is not affiliated with or endorsed by Mojang or Microsoft.
