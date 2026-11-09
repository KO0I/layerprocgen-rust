# Location connection port

`layerprocgen::location::LocationConnections::build` adapts the connection stage of the C# terrain sample's `LocationLayer.cs`. Callers provide existing stable string IDs and positions in one shared planar chart, plus a positive cell size. The source uses 360 units. RFL callers must choose a size in their chart's game units.

Locations retain their identity and position. The port does not generate, push apart, flatten, or replace buildings. It does not change climate terrain or the renderer.

Within each occupied cell, IDs are sorted and connected in a cycle, then the longest edge is removed. Two locations produce one edge; zero or one produce none. Each pair of cardinally adjacent occupied cells gets its closest location pair. Diagonal or intervening empty cells do not get a direct connection. Distance ties use canonical endpoint IDs. These rules preserve the source's connection policy while supporting arbitrary counts of existing locations rather than its fixed three generated locations per chunk. They do not promise a globally connected graph across empty cells.

Results contain canonical endpoint IDs and the half-open grid cell containing their midpoint. Midpoints use floating-point coordinates rather than the source's integer truncation. Floor division gives negative coordinates the same ownership convention as positive coordinates. Each edge appears once. `owned_by` selects connections belonging to a cell. Reordering input records preserves identical output.

`build_with_origin` additionally accepts an explicit fixed lattice origin. It changes cell ownership without translating physical site positions. The live finite-patch adapter keeps its selected sites in one cell and uses the original cycle-minus-longest rule.

Supply the full patch and a complete one-cell halo before publishing interior-cell results. An omitted cell is treated as empty. Independently selecting a different chart origin or basis changes the grid and is not a valid way to reproduce a patch. A streaming adapter must retain the same chart and cell size.

The connection output feeds the existing `path::plan_path` by resolving its endpoint IDs back to caller-owned site records. RFL should continue using its building-exclusion approach points when planning actual roads. This port provides CPU connectivity only. Live network orchestration, multiple installed roads, route retention, and junction grading belong to the subsequent cultivation and composition steps.

Run `cargo test --locked --offline --workspace` for connectivity behavior and connection-to-planner integration. The integration test uses synthetic positions and real-format IDs; it does not claim a live RFL world run.

Adapted source copyright remains Rune Skovbo Johansen, under MPL-2.0.
