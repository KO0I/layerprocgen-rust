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

LayerProCGen layers should consume that base terrain and climate data, then produce local terrain changes, placement records, and paths. The current C++ milestone provides no terrain or path algorithms and no path interchange schema. The [fixed RFL route checkpoint](route-checkpoint.md) now implements bounded routing between two existing climate-world building records, calling the existing RFL Rust sampler and exporting explicit widths and body-local path points. The frozen checkpoint remains a separate WebGL2 preview. RFL now also plans live routes through its retained Rust terrain worker, follows them through normal grounded movement, and installs shared Rust grading and an RG8 path/cliff material atlas in its existing WebGL2 pipeline. See the [live integration record](live-rfl-routes.md).

## Live integration

The RFL consumer pins this crate at `378a0359f09f06a182496df9bd6d7bb56b9c6bce`. It resolves three through eight existing climate-world building locations, generates multiple roads through Cultivation, and installs their shared core terrain effect in both collision sampling and mesh compilation. Landscape supplies RG8 path/cliff weights and exact vegetation density. Selecting a road changes only its follower. See the [network work record](live-network-plan.md) and RFL's `docs/ground-routes.md`.

Fresh terrain, materials, density, and collision effects publish together through the existing stage/commit acknowledgement. Replanning samples the clamped ungraded base, including impacts. Base edits and scene changes clear the network. Runtime networks remain ephemeral and outside climate snapshots, replay, and multiplayer.

Ordinary mesh requests now compile bounded progressive batches. Core LOD selection keeps coarse coverage until all four immediate children are ready. Complete network staging prepares all target tiles before publication. Paused refinement resumes after cancellation, and rejecting an obsolete incremental delta rebuilds the worker/GPU baseline.

## Retained network CPU modules

The location connection and [cultivation](cultivation.md) modules now provide deterministic connections over existing locations, multiple retained CPU roads, patch overlap queries, and last-lease retirement. RFL now consumes these modules through a bounded complete network snapshot. Patch-level last-lease retirement remains available to larger streaming adapters.

The [GeoGrid compositor](geogrid.md) now combines those roads with surface location pads, global lattice coordinates, full-cell underground roof protection, and explicit reservation and opening topology. Live Plate and Orbital adaptation still needs their coordinate and structure exports wired into the RFL worker and terrain mesh consumer.

The [Landscape port](landscape.md) adds physical upward normals, slope-aware climate/cliff/path weights, and deterministic grass/detail counts. It uses a checked halo and full-width cell road exclusion. RFL now consumes these immutable outputs through its existing plant system and material texture.

## Remaining sequence

1. Extend surface approaches and explicit entrance constraints, including Plate and Orbital coordinate and structure exports.
2. Define persistent network records and invalidation across climate revisions, saved worlds, replay, and multiplayer.
3. Extend generation and resource retirement to larger streamed patches and floating-origin transitions while retaining the verified ownership contract.

## Ownership contract

Layers are synchronous and shareable within one thread through `Rc`. Request tokens release retained demand on drop. External dependency registrations are weak; acquired provider leases remain alive through consumer destruction. Generation results are immutable shared values. A read callback can retire its request while keeping the value it is reading alive. Destroy hooks receive a shared view of the retired result.

Native managed executors use owned `Send` tasks and cooperative stop tokens. Scene queues and live state bindings remain local. File APIs are native-only; the browser can use the byte codec through a future host adapter.
