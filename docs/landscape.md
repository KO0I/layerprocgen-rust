# Landscape materials and vegetation density

`landscape::LandscapeRequest` derives normals, material weights, and grass/detail counts from a composed GeoGrid halo. The result keeps the terrain's final heights and cell topology. Climate color maps, plant species, plant meshes, and renderer resources remain with their existing owners.

## Input and output

A request contains a global `GridRequest` region and a 64-bit world seed. `source_request()` returns the exact source region with one vertex of halo on every side. It checks empty regions, integer overflow, and the shared GeoGrid vertex budget before allocation.

The caller fetches that halo from one complete immutable `GeoGridLayer` or `GeoGridComposition`. `build` consumes the halo and a climate plantability callback. Plantability is a finite number from zero through one, stable for each global vertex within the terrain revision. It is independent of RGB colors.

```rust,ignore
let request = LandscapeRequest { region, seed: 1610 };
let halo = composition.grid(request.source_request()?)?;
let landscape = request.build(halo, |global_vertex| {
    climate_plantability(global_vertex)
})?;
let vertex = landscape.vertex(global_vertex);
let cell = landscape.cell(global_cell);
```

For streamed terrain, acquire cultivation demand for `source_request()`, including the halo. Keep that lease until the terrain consumer unloads. A Landscape result is immutable CPU data; it does not itself acquire a cultivation lease or allocate renderer resources. The same road snapshot must supply the full halo, including roads owned outside the requested region.

The output exposes immutable vertex and cell slices, spacing, the requested region, and global-coordinate lookups. Vertices contain height, physical upward normal, material weights, and vegetation suitability. Cells contain surface presence, full-road exclusion, and a detail count. Incomplete or mismatched halo bounds are rejected. No partially derived result is published on a callback error.

GeoGrid buffers and spacing are now read-only as well. Use `vertices()`, `cells()`, `spacing()`, `vertex()`, and `cell()`. The lattice, heights, protected openings, and road exclusion metadata cannot be edited independently after construction. Compile-fail checks protect these ownership rules.

## Normals and materials

Normals use centered differences from the global neighbor vertices. The vector before normalization is `[height_left - height_right, 2 * spacing, height_down - height_up]`. This gives physical upward normals for a local Y-up height field. It intentionally reverses the C# sample's lateral signs. Its Y-based cliff classification is unchanged.

A vertex is cliff material when `normal.y < 0.65`. Cliff vertices have zero path weight and full cliff weight. Otherwise, path weight is the combined location and road coverage, and the remaining weight belongs to the existing climate base color. The three nonnegative weights sum to one. The port does not replace the climate palette with the sample's grass color.

The pass never lowers or renormalizes heights. In particular, it does not copy the Unity edge-lowering pass, which could invalidate underground roof cover. Neighboring patches evaluate the same global vertices with the same halo, so shared normals and weights match exactly even at negative indices and fractional spacing.

## Vegetation counts

Vertex vegetation suitability is the climate material weight times climate plantability. A nonpassable vertex, opening, or reserved surface has zero suitability.

Each cell uses its four corner suitability values. If the average is greater than 0.4, its density is the maximum corner value times 10 plus hash jitter, rounded to the nearest even integer at a tie. Otherwise it is zero. Counts are bounded to zero through ten. A zero climate suitability at one corner does not by itself reject the whole cell; the four-corner average still controls density.

Cells touched by a road, opening, or reserved surface always have zero detail count. Cells with any nonpassable or non-solid corner are also excluded conservatively. A buried room alone is solid terrain and does not suppress vegetation when its roof cover is valid.

GeoGrid computes road/cell occupancy from canonical road segments and their complete surface, shoulder, and material widths. The segment is tested against the cell rectangle expanded by that radius. This is conservative near rounded corners. It detects a thin road or point road entirely between vertices, where all vertex coverage values can be zero. Whole-cell exclusion avoids placing detail instances across those narrow roads.

Hash input includes the full signed 64-bit global X and Z indices, both halves of the 64-bit world seed, and detail stream tag 9. Jitter lies in `[-0.5, 0.5)`. This deliberately replaces the C# patch-local random pattern. Reordering or repartitioning requests preserves counts for the same global cell. Finite hashes do not imply collision-free IDs.

Counts are per grid cell, not per square metre. This is the fine-detail policy from the source. LOD density conversion and actual instance placement remain adapter work. The C# sample supplies null tree instances, so no tree generator is claimed here.

## Verification and integration boundary

Run:

```sh
cargo test --locked --offline --test landscape --test geogrid
cargo test --locked --offline --workspace
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo check --locked --offline --workspace --target wasm32-unknown-unknown
node tools/check-route-fixture.mjs
```

Behavior checks compare split and combined patches, cliff classification, independent climate suitability, high seed and coordinate bits, thin and point roads, water boundaries, protected roofs, and narrow openings. A real-terrain case reads the frozen RFL native elevation export and verifies shared-edge materials and unchanged heights. Its plantability is a controlled test input; it does not claim live climate vegetation consumption.

The existing RFL climate generator and WebGL2 renderer were not changed by this CPU port. Live multi-road terrain publication and consumption by RFL's existing plant system remain the next integration checkpoint. TerrainLODManager coverage selection follows that integration. Plate and Orbital adapters must still preserve their coordinate frames and the protected structure and opening records documented in [GeoGrid composition](geogrid.md).

The source is `Samples/TerrainSample/Scripts/Generation/Layers/LandscapeLayer.cs`, copyright Rune Skovbo Johansen, under MPL-2.0. The design and review record is [Landscape material and vegetation work record](landscape-plan.md).
