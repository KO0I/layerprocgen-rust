# layerprocgen-rust

Rust implementation of [LayerProCGen](https://github.com/KO0I/LayerProCGen)'s deterministic generation core, with a separate Rust/WebAssembly viewer using WebGL2.

## Implemented

The core crate has no third-party dependencies. It includes coordinate arithmetic, half-open bounds, fixed-point values, RandomHash, floating-origin notifications, retained chunk requests, level dependencies, rollback, reusable records, stale-handle checks, action queues, object pools, work tracking, native executors, save-state encoding, and live state bindings.

The point-generation and cube-spawning sample preserves the C++ baseline's seed, chunk sizes, positions, retained overlaps, FIFO scene actions, and shutdown behavior. The browser viewer provides movement, zoom, queue pause, stop, and restart. Generation and rendering live in separate crates.

The Rust path planner adapts the C# terrain sample's search and spline tangents to an immutable terrain callback. A [repeatable RFL checkpoint](docs/route-checkpoint.md) connects two existing building locations on a fixed climate patch, using RFL's actual elevation sampler and explicit surface, shoulder, and material widths. The viewer can display this route with **Climate path** or `/?scene=route`. RFL now installs multi-road networks, follows a selected road on foot, grades terrain through shared Rust effects, consumes Landscape materials and density, and progressively refines terrain through its existing WebGL2 renderer. See the [live integration record](docs/live-rfl-routes.md) and [remaining work and climate boundary](docs/port-plan.md).

The [location connection port](docs/location-connections.md) builds deterministic connections from existing site IDs and chart positions, with unique midpoint ownership. It preserves the C# within-cell and nearest cardinal-neighbor rules.

The [cultivation port](docs/cultivation.md) generates and retains multiple roads from those connections. Patch leases share road payloads, support width-aware overlap queries, and retire roads on the last unload.

The [GeoGrid composition port](docs/geogrid.md) combines location and road effects on a global grid. It preserves shared patch vertices and protects buried roofs, reserved surface geometry, and intentional openings.

The [Landscape port](docs/landscape.md) derives shared-edge normals, slope-aware material weights, and globally seeded vegetation density from that composed terrain. Climate colors and plant assets stay with their existing owners.

The [LOD coverage selector](docs/lod.md) keeps coarse terrain active until all four immediate children are ready. It returns stable CPU tile identities and sparse missing coverage without owning meshes or graphics resources.

## Build and check

Use Rust 1.88 or later. The viewer's cached dependencies are needed for workspace checks; fetch them once when online.

```sh
cargo fetch --locked
cargo build --locked --offline
cargo test --locked --offline --workspace
node tools/check-route-fixture.mjs
cargo fmt --all --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo run --locked --offline --example headless_simple
```

## Run the WebGL2 viewer

Install the `wasm32-unknown-unknown` Rust target and `wasm-bindgen-cli` version `0.2.128`.

```sh
python3 tools/build-web.py
python3 -m http.server 4173 --bind 127.0.0.1 --directory web
```

Open <http://127.0.0.1:4173>. Drag or use WASD and arrow keys to move. Pinch or scroll to zoom, twist to rotate, and slide two fingers vertically to tilt. Press **Climate path** to view the RFL route.

The [viewer guide](docs/webgl.md) includes browser verification and executable C++ comparison commands. The browser check verifies a WebGL2 context with WebGPU unavailable, rendered pixels, real input, streaming, deterministic return, resource retirement, and page lifetime.

## Integration direction

The Rust core produces CPU data and has no graphics backend requirement. For the climate-based game workflow, climate output supplies the base terrain. Generation layers can add detail, paths, and placement records, then return CPU data to the game's existing WebGL2 renderer. The standalone viewer is a validation example.

Layer callbacks run synchronously. The scene action queue and state bindings belong to one thread. Native executors accept owned `Send` jobs separately. A worker integration must transfer owned CPU results to the scene owner.

## Provenance and verification

The translation reference is C++ commit `8fb913c7667e576b8f8cdbb62295d2e2a222b46b`. The original C# source remains the upstream behavioral authority. The executable comparison checks 759 rows covering hashes, fixed-point conversions, generated positions, and a save-state byte fixture. This does not claim full Unity equivalence.

The [migration record](docs/migration.md) documents ownership choices, intentional changes, and checks. The [decision trail](docs/decisions.tsv) records the run's checkpoints.

## License

MPL-2.0. See [LICENSE](LICENSE) and [Third Party Notices.md](Third%20Party%20Notices.md). Translated algorithms retain their source attribution.
