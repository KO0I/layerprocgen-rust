# RFL climate path checkpoint

Rust connects two existing RFL building records on one frozen climate-world patch. It samples RFL's existing `SurfaceTerrain::elevation`, searches a bounded corridor, smooths across validated terrain, preserves exact endpoints, and exports explicit widths. The WebGL2 preview displays the exported route on the same patch.

## Run and reproduce

Build the viewer and serve `web/` as described in [the viewer guide](webgl.md). Open `/?scene=route`, or press **Climate path** in the points viewer. Drag to pan, pinch to zoom, twist to rotate, and slide two fingers vertically to tilt. Home fits both endpoints to the viewport. Cyan marks the start; red marks the goal; gold shows the surface width.

The committed fixture and preview can be checked without an RFL checkout:

```sh
cargo test --locked --offline --workspace
node tools/check-route-fixture.mjs
python3 tools/build-web.py
node tools/check-web.mjs
```

Reproduce the route using the actual RFL sampler and independently compare its elevations with RFL's JavaScript sampler:

```sh
node tools/export-rfl-route.mjs --rfl ../rasd-fastwalker-league-js --check
```

This builds an isolated adapter under `target/rfl-route-bridge/`. It reads the RFL checkout without modifying it. RFL's native crate requires Rust 1.90 or later; the standalone core and viewer require 1.88. The adapter also needs cached `serde_json` 1.0.149 and its dependencies. The core crate retains zero third-party dependencies.

`--check` writes only under `target/` and compares regenerated route and terrain bytes with the frozen fixture. The manifest's selected source-file hashes and all inputs must match. An unrelated change to RFL's HEAD is allowed when those source hashes and output bytes still match; the frozen manifest retains its original revision for provenance. To deliberately replace the fixture and browser data, omit `--check`.

## Frozen world and locations

The input is RFL's version-one climate snapshot from the existing Quick fixture, seed `1610`, radius `1000`, sea level `0`, and a `256 × 128` height raster. Its full export SHA-256 is `e9225cfd01683391d78899510e89d213351d563184dc77d95c9b27d2dee4db0a`.

| Role | Existing site ID | Group |
| --- | --- | --- |
| Start | `0:21:21:0:0` | `alien-buildings` |
| Goal | `0:18:22:0:4` | `alien-buildings` |

Both records come from RFL's ordinary `streamRegion` / `generateTile` flow. Their original IDs and body-local positions are preserved. They are generated building centers, not authored town names or entrance portals.

The patch uses a gnomonic chart about the midpoint. The manifest stores its orthonormal center, right, and forward vectors. A chart point `(x,z)` maps to the direction `normalize(center + (right*x + forward*z)/radius)`. The sampler returns elevation above the body's radius. Coordinates and widths use RFL world units; the existing snapshot conversion has already divided metre heights by `6371`.

## Planner and output contract

The dependency-free API is `layerprocgen::path::plan_path(sample, start, goal, options)`. The callback supplies elevation, nonnegative traversal cost, and passability from an immutable snapshot. `None` marks unavailable terrain. Caches belong to one request, so a later request cannot reuse a previous world's heights.

The search adapts the original C# `TerrainPathFinder`'s 24 lattice moves and slope penalty. Deterministic heap ties and a validated goal connector preserve arbitrary exact endpoints. Search and sampling have explicit work budgets. Every lookup, including a cache hit, consumes the sampling budget.

The `TerrainPath` spline tangents are adapted to Rust cubic curves. Every proposed chord is checked across the full footprint for passability and longitudinal and transverse grade. Smoothing that leaves legal terrain or materially increases traversal cost falls back to the validated, densely sampled polyline. Output heights always come from the terrain callback.

| Width field | Checkpoint value | Meaning |
| --- | --- | --- |
| `surface` | `4` | Full walkable surface width |
| `shoulder` | `1` | Additional width on each side |
| `material` | `5` | Full material footprint width |

The validated footprint uses the larger of the surface plus shoulders and the material width: `6` units here. Planning uses grid spacing `2`, terrain sample spacing `1`, corridor radius `30`, maximum grade `0.6`, at most `10,000` expansions, and at most `400,000` sample lookups.

`fixtures/rfl-route/route.json` contains the raw and smoothed `[chart x, elevation, chart z]` points, corresponding `bodyLocalPoints`, site IDs, widths, options, and diagnostic counts. The body-local points are `direction * (radius + elevation)` and can be passed to a future RFL path consumer without depending on the preview camera. The manifest and binary arrays retain the climate input, site records, coordinate frame, sampled display grid, and input hashes. `web/route-checkpoint.json` bundles the verified route and display grid for the viewer.

## Verification and limits

The fixed fixture produces 36 raw nodes and 261 smoothed points. Repeating the request and recreating the terrain sampler produce identical results. The native check compares all 8,385 JavaScript display-grid elevations against the existing Rust sampler, with a maximum difference below `3e-15` world units. It independently probes the returned footprint at quarter-unit spacing and checks dry terrain, grade, exact site elevations, and body-local endpoints.

Behavioral tests cover water crossing, shoulder clearance, a detour around blocked terrain, traversal-cost preservation through smoothing, exact endpoints, changed snapshots, grade limits, numeric rounding, malformed samples, and work budgets. Browser verification checks a continuous gold centerline at 15°, 45°, and 80° tilt, both markers in a mobile viewport, mode switching, direct route entry, route stop/restart, and touch input. Visual inspection exposed a camera depth degeneracy at 45°; the new pixel continuity assertion failed before the orthogonal depth basis was applied and passes with complete centerline coverage afterward. The existing points viewer checks remain in the same suite.

Validation is discrete. Obstacles narrower than the configured sampling interval can be missed. This checkpoint covers terrain constraints within one bounded chart. It does not avoid building volumes, find entrance portals, deform the ground, control an NPC, or connect to RFL's live worker lifecycle. The display grid is an approximation for viewing; it is never the planner's elevation authority. The preview exaggerates elevation by `80` and lifts the path slightly to avoid depth flicker; the exported heights retain their original units.

## Design record

A callback planner was selected over a planner that owns a sampled raster. This lets the adapter query RFL's existing elevation implementation directly and avoids introducing another interpolation contract. The planner and viewer remain separate from RFL's rendering code.

The original C# search and smoothing sources were inspected before choosing the data and sampler boundary. Parallel review found numeric tolerance and work-budget edge cases; the fixes have behavioral regression checks. A local Hermes review did not locate the external upstream source and returned no usable verdict, so Codex inspected the source directly. No local review verdict is claimed. Cross-family judging is unavailable in the configured Codex model pool.

All workspace writes were performed by the parent agent; read-only workers returned drafts and findings. The RFL source was not modified by this checkpoint.
