# Rust port status and remaining work

## Reference baseline

The C++ translation reference is commit `8fb913c7667e576b8f8cdbb62295d2e2a222b46b`. Its 79 existing tests pass. The original C# source remains the upstream behavioral authority. No Unity execution is claimed.

## Implemented milestone

The Rust core implements deterministic numeric types, bounds, RandomHash, floating origin, retained synchronous chunk demand, previous-level and external dependencies, release and rollback, pooled records, stale-handle checks, scene action queues, resettable object pools, work tracking, native executors, save-state bytes, file replacement, and live state bindings.

The headless points sample and separate Rust/WebAssembly WebGL2 viewer use the same generation and lifecycle code. The viewer owns its graphics context and buffer. The core owns CPU results. The WebGL2 route replaces the earlier raylib plan.

## Climate-based terrain and path integration

Use the climate workbench's output as the authoritative base map. The Rust generation core does not require a WebGL version. The game renderer continues to own its WebGL2 resources.

RFL already separates `native/world` terrain algorithms, the `native/terrain-wasm` worker adapter, and WebGL presentation. Its existing `BaseTerrain::ClimateMap` stores width, rows, and owned `Vec<f32>` heights. See the source project's `docs/decisions/0013-independent-terrain-compiler.md`.

The standalone climate workbench's scientific export uses Gaussian latitude coordinates and heights in metres. The game's version-one climate snapshot uses a regular longitude/latitude raster and game units. These are different formats. Before adding an importer, define which export is the input, its coordinates and units, how it wraps longitude, and how ocean elevations are represented. The existing game snapshot adapter is a reference for those conversions.

LayerProCGen layers should consume that base terrain and climate data, then produce local terrain changes, placement records, and paths. The current C++ milestone provides no terrain or path algorithms and no path interchange schema. The [fixed RFL route checkpoint](route-checkpoint.md) now implements bounded routing between two existing climate-world building records, calling the existing RFL Rust sampler and exporting explicit widths and body-local path points. The frozen checkpoint remains a separate WebGL2 preview. RFL now also plans live routes through its retained Rust terrain worker, follows them through normal grounded movement, and installs shared Rust grading and an R8 material mask in its existing WebGL2 pipeline. See the [live integration record](live-rfl-routes.md).

## Live integration

The RFL consumer pins this crate at `fb0f91e788167e5abe83aa5be5d224c6c6670c25`. It uses RFL's current climate snapshot and elevation sampler, resolves two existing building locations, and finds external approach points outside their projected meshes. A dedicated ground follower moves the actor physically along the route. The terrain effect is implemented in RFL's native world crate and shared with main-thread collision sampling through Wasm.

Fresh terrain resources are staged before publication. Scene epochs, body identity, base revisions, and publication acknowledgements reject stale work. Geometry, collision heights, and the material mask change together. Route clearing and replayed planning preserve the original climate arrays. Native, Wasm parity, and real browser checks cover this slice.

Each body supports one bounded runtime road in solo mode. Replanning replaces it. Base edits and scene changes clear it. The result is not persisted in climate snapshots or replay data. The original checkpoint records its frozen source hashes; live integration checks use the current RFL source rather than rewriting that historical fixture.

## Remaining sequence

1. Add terrain-detail and placement layers against the existing climate baseline. Check continuity across chunk and longitude boundaries.
2. Extend bounded paths into connected networks with application-specific connectivity and entrance constraints.
3. Define persistent route records and invalidation across climate revisions, saved worlds, replay, and multiplayer.
4. Extend generation and resource retirement to larger streamed patches and floating-origin transitions while retaining the verified ownership contract.

## Ownership contract

Layers are synchronous and shareable within one thread through `Rc`. Request tokens release retained demand on drop. External dependency registrations are weak; acquired provider leases remain alive through consumer destruction. Generation results are immutable shared values. A read callback can retire its request while keeping the value it is reading alive. Destroy hooks receive a shared view of the retired result.

Native managed executors use owned `Send` tasks and cooperative stop tokens. Scene queues and live state bindings remain local. File APIs are native-only; the browser can use the byte codec through a future host adapter.
