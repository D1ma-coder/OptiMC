# Contributing

OptiMC is experimental. Check [current status](docs/STATUS.md) and [protocol coverage](docs/COVERAGE.md) before starting work. Small changes with clear behavior and reproducible evidence are easiest to review.

## Development

Use stable Rust and keep `Cargo.lock` in version control.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Default tests do not open a Vulkan window. Some tests need loopback TCP. Asset-dependent and GPU tests are ignored by default; do not enable them as part of routine CI. Review the [GPU incident](docs/GPU-INCIDENT-2026-10-20.md) before changing presentation or running graphics experiments.

## Changes

- Describe the concrete problem, resulting behavior and validation in your pull request.
- For protocol changes, identify the target version and include a bounded decoder plus malformed-input coverage where useful.
- Preserve server authority and world/teleport generation fences.
- Reference comparisons must distinguish byte/CPU validation from visual or live-server testing.
- Do not commit personal profiles, server journals, credentials, downloaded JARs, textures or build artifacts.
- Preserve upstream attribution and license terms when porting code. Update `THIRD_PARTY_NOTICES.md` and the relevant provenance manifest.

## Bug reports

Include the commit, OS/GPU, server version, steps to reproduce and relevant log excerpts. Remove chat, addresses and account information you do not want public. A packet name alone does not prove complete semantic support; note whether it caused a visible problem or disconnect.
