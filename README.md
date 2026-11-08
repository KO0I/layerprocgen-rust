# layerprocgen-rust

A Rust port of [LayerProCGen](https://github.com/KO0I/LayerProCGen), a framework for layered procedural world generation.

The goal is an engine-independent Rust library for deterministic generation, chunk dependencies, and world streaming. Rendering integrations can build on the library without tying its core to a particular graphics library or game engine.

## Status

The repository contains a dependency-free Rust crate, CI checks, and a [port plan](docs/port-plan.md). Generation algorithms and the chunk runtime are not implemented yet.

## Planned capabilities

- Deterministic coordinate arithmetic, bounds, fixed-point values, and hashing.
- Chunk requests, dependency tracking, loading, and unloading.
- Explicit ownership of generated data, with safe cancellation and shutdown.
- Headless samples and behavioral tests for repeatable generation.
- Terrain generation, floating origins, and streaming.

These are implementation goals, not capabilities of the current scaffold.

## Build

Use a stable Rust toolchain with edition 2024 support.

```sh
cargo build --offline
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The initial crate has no dependencies. There are no behavioral tests yet; `cargo test` currently checks compilation and runs zero tests.

## Project origins

LayerProCGen's original implementation is the behavioral reference for this port. The [port plan](docs/port-plan.md) records source baselines and the implementation sequence.

This repository starts with new Rust scaffolding. Preserve source attribution and applicable license notices when translating algorithms into Rust.

## License

MPL-2.0. See [LICENSE](LICENSE) and [Third Party Notices.md](Third%20Party%20Notices.md). The notices are retained from the reference project for future translations; their presence does not mean those components are already implemented in Rust.
