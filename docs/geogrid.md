# GeoGrid composition

`geogrid::GeoGridLayer` composes surface locations over an existing terrain sampler. `GeoGridComposition` adds multiple roads as a second stage. The layer uses one planar chart, one immutable terrain revision, and one global grid spacing.

The original C# GeoGrid supplies location-deformed terrain to Cultivation. Landscape later applies location and road effects to the final terrain. This port preserves that distinction. Roads never become the terrain on which subsequent roads are planned.

## Composition data

`SurfacePad` defines an explicit surface target height and full surface, shoulder, and material widths. A buried room is a `SurfaceProtection::BuriedVolume`, not a pad at its floor elevation.

Location effects and roads each blend as a group. Effects are sorted by stable IDs. Road IDs are canonical endpoint pairs. Repeated identical road geometry contributes once. Conflicting geometry with the same road ID is rejected. Road bounds are recomputed from the actual points and widths.

For a segment, let `inner` be half the surface width and `outer` be `inner + shoulder`. At distance `d` from the segment, the C# weight is `max(0, (outer-d)/shoulder)^8`. Influence is the smoothstep of that ratio clamped to zero through one. The segment target height interpolates along its line, with the source's extrapolation limited to `[-4,5]`.

Each stage averages target heights and influences using those weights, then blends from the incoming height by the averaged influence. Logarithmic rescaling avoids overflowing the eighth power. Zero shoulders use a hard surface boundary. Repeated points and single-point effects remain finite. The source's segment-count weighting is retained, so changing road tessellation can change intersection grading.

The material adaptation follows RFL's existing explicit widths. Coverage uses a maximum union, independent of the grading radius. It can therefore extend beyond the shoulder. The feather is half a chart unit, limited to half the material width. Road coverage replaces the corresponding fraction of location coverage. These rules deliberately differ from C# summed splats and its outer-radius early exit.

Nearest-location metadata contains the vector toward the location center and signed distance to its surface edge. The planning sample adds the source's location cost, which falls from 20 at distance -6 to zero at distance 6 in chart units. The underlying terrain's cost and passability remain inputs.

## Global grids

`GridRequest` contains a global integer origin and cell counts. Vertex coordinates are calculated as `global_index * spacing`. Incident cell bounds use neighboring global indices as well. Patches do not calculate world coordinates from their own local origins. This includes negative coordinates and fractional spacing.

`grid(request)` returns row-major vertices and cell topology. `vertices()`, `cells()`, and `spacing()` expose read-only snapshot data. `cell_has_road(global_cell)` provides conservative full-width road intersection, including roads missed by vertex masks. There is one more vertex than cell along each axis. `vertex(global_point)` and `cell(global_point)` query the result. Identical snapshots produce identical shared vertices regardless of request order.

`terrain_sample(point)` interpolates the locations-only global grid using its two terrain triangles. It returns a `TerrainSample` for the existing path planner. The caller can propagate a sampling error separately or map it to unavailable terrain at the planner boundary.

The layer contains the complete location and protection set. It does not accept a different contributor list for each terrain patch. A complete location graph and its required halo remain the caller's responsibility. This version computes immutable patch grids without a shared tile cache.

## Retained cultivation integration

The intended call sequence is:

```rust,ignore
let locations = GeoGridLayer::new(spacing, pads, protections, base_sampler)?;
let mut cultivation = CultivationLayer::new(
    &sites,
    &connections,
    options,
    |point| locations.terrain_sample(point).ok(),
)?;
let terrain_patch = locations.compose_patch(&mut cultivation, request)?;
let shared_vertex = terrain_patch.grid.vertex(global_point);
drop(terrain_patch);
```

`compose_patch` acquires the cultivation demand covering the patch, captures the complete retained road set, and generates final terrain. Candidate road envelopes already include their search corridor and full widths. A road owned outside the patch still contributes at the shared border. `GeoGridPatch` keeps that demand until it unloads. A failed composition drops its new demand and preserves existing leases.

`with_roads(roads)` creates an immutable final snapshot for explicit CPU queries. Its caller supplies a complete road snapshot. `compose_patch` is the operation that combines road acquisition, contributor completeness, and patch lifetime. Neither operation mutates the pre-route layer.

## Owned scalar effects

`GeoGridComposition::surface_effects()` exports immutable location and road geometry for a consumer that already owns its base-height sampler. `SurfaceEffects::sample(point, source_height)` uses the same blend accumulator as grid generation. A bounded spatial index preserves canonical segment order and avoids visiting the whole network for each terrain query. The snapshot owns no climate arrays or callback.

This export rejects every composition containing protections with `ProtectedEffectsRequireGrid`. Buried cover and opening topology require incident-cell support and the full grid consumer. A scalar height adapter cannot preserve that contract.

## Underground structures and openings

`BuriedVolume` contains a stable ID, a conservative projected footprint, floor, roof, and required cover. For every terrain cell touched by the footprint, all four corner heights must be at least `roof + cover`. The maximum requirement wins where buried volumes overlap. Linear triangle interpolation then preserves that lower bound throughout the touched cell.

The constraint is evaluated after all location and road grading. `GeoSample` keeps the requested height and the minimum cover height. `cover_limited()` reports a cut that could not be accepted. Callers can use that result to reject or replan a road whose requested grade changed.

If the unmodified base is already below the required cover, sampling returns `InsufficientCover`. The compositor does not change climate generation to repair a preexisting structural conflict.

`Reserved` preserves source terrain and clears grade materials around special geometry such as hull extensions, canyons with authoritative floor meshes, and surface shafts. `Opening` also marks touched cells as holes. A narrow opening entirely between vertices still appears in cell topology. Both kinds are non-walkable through the ordinary terrain planner. Full-width road segments touching either kind are rejected when a road snapshot is constructed. The rectangular protection can conservatively exclude extra space around a polygon or rounded road end.

Numeric height remains available under holes, matching RFL's distinction between `terrainAt` and its cut `surfaceMesh`. A mesh consumer must honor the cell topology. The generic grid marks whole touched cells; RFL's exact polygon clipping remains separate. `openToSky` does not automatically mean a hole. A canyon with a floor mesh is a reservation. An adapter classifies an intentional surface void as an opening.

RFL Interiors uses metres, local XZ footprints, and positive Y up. Prefer each `roomVolumes` prism to the nominal room box because terraced rooms have different ceiling heights. A shaft or lift is an explicit reservation rather than an underground location flattened onto the surface.

Plate flight geometry normalizes its local metres by 28,000. Live Orbital flight terrain uses inward-facing cylindrical coordinates. These need explicit coordinate adapters before this planar compositor can affect those worlds. Existing ship collision triangles and Interiors geometry stay authoritative. RFL now consumes owned scalar effects for bounded climate-world networks. Plate and Orbital protection/topology consumers remain separate.

## Limits and verification

Coordinates, heights, and individual widths are limited to magnitude `1e12` chart units. Construction accepts up to 4,096 pads, protections, and supplied roads. A road snapshot accepts up to 262,144 total segments. A grid accepts up to 1,048,576 vertices and 20,000,000 estimated contributor operations. Reservation intersection work has the same limit. Validation precedes enumeration and oversized road copying. Rebuild the layer when the terrain revision, chart, locations, or protections change.

Run:

```sh
cargo test --locked --offline --test geogrid
cargo test --locked --offline --workspace
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo check --locked --offline --workspace --target wasm32-unknown-unknown
node tools/export-rfl-geogrid-fixture.mjs --check
```

The RFL fixture checker reads the sibling checkout by default. Pass `--rfl /path/to/rasd-fastwalker-league-js` for another location. Without `--check`, the exporter rewrites the fixture and provenance for review. The fixture records two real enclosed prisms generated with seed `GARDEN-07`, zero additional room density, and zero atriums. Its base heights are conservative source terrain minima, not copied climate or terrain arrays. Source working-file hashes accompany the RFL HEAD because the checkout can contain uncommitted work.

The Rust tests cover weighted intersections, stage isolation, material rules, contributor order, negative and fractional patch borders, duplicate road identities, full-cell cover, narrow openings, shared demand, failed publication, and budget boundaries. The fractional-boundary test failed before vertex support changed to neighboring global lattice indices. The real Plate and Orbital fixtures verify cuts stop above existing roofs and never flatten to room floors.

The adapted C# `GeoGridLayer`, `TerrainPathDeformation`, and `PathsBurstMethod` retain Rune Skovbo Johansen's MPL-2.0 attribution. The design and review record is [GeoGrid composition work record](geogrid-plan.md).
