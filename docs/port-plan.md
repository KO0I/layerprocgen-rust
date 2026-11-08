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

LayerProCGen layers should consume that base terrain and climate data, then produce local terrain changes, placement records, and paths. The current C++ milestone provides no terrain or path algorithms and no path interchange schema. The [fixed RFL route checkpoint](route-checkpoint.md) now implements bounded routing between two existing climate-world building records, calling the existing RFL Rust sampler and exporting explicit widths and body-local path points. The route is previewed separately in WebGL2. Live game worker consumption and terrain deformation remain unimplemented.

## Remaining sequence

1. Extend the verified fixed-world adapter to accept live versioned climate snapshots. Preserve macro terrain and sea/land classification.
2. Port or implement terrain-detail and placement layers against that base. Check continuity across chunk and longitude boundaries.
3. Extend the verified bounded path planner with application-specific connectivity, building/entrance constraints, and terrain modifications.
4. Connect generated CPU results to the existing game resource adapter. Check stale responses, replacement, cancellation, floating origins, and cleanup.
5. Add bounded background generation where needed. Keep worker-owned CPU state separate from renderer-owned GPU resources.

## Ownership contract

Layers are synchronous and shareable within one thread through `Rc`. Request tokens release retained demand on drop. External dependency registrations are weak; acquired provider leases remain alive through consumer destruction. Generation results are immutable shared values. A read callback can retire its request while keeping the value it is reading alive. Destroy hooks receive a shared view of the retired result.

Native managed executors use owned `Send` tasks and cooperative stop tokens. Scene queues and live state bindings remain local. File APIs are native-only; the browser can use the byte codec through a future host adapter.
