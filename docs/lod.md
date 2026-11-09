# Terrain LOD coverage

`lod::select_coverage(available, roots)` adapts the C# sample's `TerrainLODManager` activation rule to immutable CPU tile identities. The caller owns mesh compilation, cache eviction, and atomic renderer publication.

`LodTile` has a domain, level, and signed X/Z indices. Level zero is finest. Each parent covers four immediate children at the next finer level. Domains keep separate charts or cube faces independent. Parent coordinates use Euclidean floor division, including negative indices.

An available parent stays active until all four immediate children are available. Once they are ready, selection recurses into those children. Grandchildren cannot substitute for an absent immediate child while its parent is available. If no parent exists, available descendants can cover individual branches. Uncovered branches appear in `missing` without enumerating an empty finest-level rectangle.

Requested roots define the coverage boundary and must not overlap. Available duplicates coalesce. Coarser tiles and outside domains do not contribute. Active and missing outputs are canonically sorted. Repeating a query has no side effects.

The boundary accepts at most 8,192 available tiles and 8,192 roots, levels zero through thirty, and finest descendant coordinates that fit i32. At most 262,144 ancestry and traversal operations are allowed. Invalid inputs and exhausted work budgets return explicit errors.

RFL's cube-face terrain keys use level zero for the coarsest tile. Its adapter must reverse depth around the request's maximum depth and use each face as a domain. A progressive compiler can retain root and intermediate meshes while preparing finer batches. It must publish the selected set together so a coarse tile retires only with complete replacement coverage.

Run `cargo test --locked --offline --test lod`. The tests cover partial children, missing intermediate children with complete grandchildren, nested replacement, sparse gaps, negative coordinates, domain isolation, stable ordering, invalid roots, coordinate overflow, and work limits. Unity terrain objects and profiling hooks are not ported.

The adapted `TerrainLODManager` algorithm retains Rune Skovbo Johansen's MPL-2.0 attribution. Live RFL progressive consumption is tracked in the [network work record](live-network-plan.md).
