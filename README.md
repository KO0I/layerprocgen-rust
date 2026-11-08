# layerprocgen-rust

A Rust port of LayerProCGen, with an engine-independent generation library and a planned raylib viewer.

## Status

Repository scaffold only. No generation algorithms, chunk runtime, or raylib viewer have been ported yet. The working C++ implementation remains available as a reference; its tests do not establish Rust parity.

## Build

Use a stable Rust toolchain with edition 2024 support.

```sh
cargo build --offline
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The initial crate has no dependencies. Tests currently verify only that the crate builds; there are no behavioral tests yet.

## References and provenance

- [Original C#/Unity source](https://github.com/KO0I/LayerProCGen/tree/cc9905d).
- [C++20 port baseline](https://github.com/KO0I/LayerProCGen/tree/8fb913c), branch `feature/cpp20-raylib-port`.
- [Port plan](docs/port-plan.md).

This repository starts with new Rust scaffolding rather than copying Unity assets or C++ build artifacts. Source history and contributor attribution remain available in the linked reference repository. When translating code, retain applicable copyright and license notices.

## License

MPL-2.0. See [LICENSE](LICENSE) and [Third Party Notices.md](Third%20Party%20Notices.md). The notices are retained from the reference project for future translations; their presence does not mean those components are already implemented in Rust.
