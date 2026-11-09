# Live RFL network and LOD work record

## Goal

Install a bounded network between at least three existing climate-world locations, with at least two roads coexisting in terrain, collision, materials, and plant placement. Selecting a road changes only the follower. Then port C# LOD coverage selection and consume it in progressive Rust terrain publication. Climate generation and WebGL2 remain authoritative.

## Work list

- [x] Read poteto-mode principles and ground core/native/runtime ownership.
- [x] Phase A: Frame.
- [x] Phase B: Design the workflow.
- [x] Phase C: Run the loop.
- [x] Core owned surface effects and explicit location grid origin, verified against GeoGrid.
- [x] Verified dependency checkpoint for portable RFL Git pin.
- [x] Native and Wasm whole-network planning, composition, materials, and density.
- [x] Runtime network publication, road selection, and plant/material consumption.
- [x] Real WebGL2 multi-road and stale-result verification.
- [x] Rust LOD coverage selection and progressive terrain integration.
- [x] Real coverage transitions and full workspace checks.
- [x] Phase D: Keep the audit trail.
- [x] Phase E: Verify and hand back.

## Throughput checkpoint

Two repositories are involved. At task start, the core had verified uncommitted Landscape changes. RFL has unrelated ongoing Interiors and rendering work that stays untouched. The parent owns integration. Source readers and two architecture candidates are read-only. One bounded local Hermes worker owns only `src/lod.rs` and `tests/lod.rs`; the parent inspects and verifies its result. No shared file writers or model switches are needed.

The riskiest seam is the base sampler. A borrowing GeoGridComposition cannot be stored in SurfaceTerrain or sent to the main Wasm instance. A core-owned SurfaceEffects snapshot reuses the same blend kernel and owns only canonical surface geometry. It explicitly rejects protected snapshots because a scalar base-height callback cannot reproduce whole-cell cover semantics. Existing live routes remain climate-only until Plate/Orbital adapters exist.

The immutable network record, selected follower road, and pending publication transaction are separate state. One complete network is staged and published once. The existing scene/body/base/request guards and acknowledgement remain. Failure before publication preserves old state; failure afterward uses existing worker recovery.

The real fixture uses existing building IDs `0:21:21:0:0`, `0:18:22:0:4`, and `0:16:33:4:0`. Bounded discovery must expand beyond the former two-site radius. A common orthonormal chart and explicit location grid origin produce deterministic finite-patch connectivity. Each site has one mesh-clear external approach shared by incident roads.

Core surface effects are checked before RFL depends on them. The dependency must name an immutable available Git revision, not a sibling path. A core checkpoint is necessary after verification. The bundled deslop and control-ui skills are not installed; scoped source cleanup and the committed Chromium driver provide the available verification route.

## Architecture selection

Candidate A uses an owned analytic core effect. Candidate B uses a height-delta raster. The base is A because main and worker can apply it directly to their existing analytic climate height without resolution-dependent collision changes. B contributes explicit aggregate work/byte budgets and zero-change outside the influence envelope. B's delta interpolation cannot inherit the core's buried-roof guarantee for nonlinear bases, reinforcing the climate-only live gate.

Prepare a spatial index in the owned effect while preserving canonical segment order. Both indexed and ordinary grid queries use one blend accumulator. Delete RFL's independent segment grading when migrating callers. Material/density data is derived from real Landscape batches with one shared global lattice and exact halos. Existing climate life supplies plantability; RGB never supplies it.

The existing texture on unit eight carries path and cliff weights. Exact cell counts remain a nearest-cell CPU field for deterministic count/10 thinning of existing plant candidates. Buildings retain their existing generation policy. The seven-float terrain vertex ABI stays in place.

LOD follows the live-network proof. The core selector preserves a coarse available tile until all four immediate children are ready. It handles negative indices and sparse missing coverage. RFL continues to own mesh compilation and GPU publication. Progressive compilation retains ancestors while preparing bounded finer batches; a staged network uses a complete batch before publication.

The architecture candidates and reviewers are GPT-family models. No independent cross-family verdict is claimed.

## Core dependency checkpoint

The local LOD worker exhausted its six-turn budget without code. A Codex draft was inspected and integrated by the parent after the child write approval stalled. The actual core files pass 118 checks, workspace Clippy, formatting, and the Wasm workspace check. These include the prior Landscape checkpoint, owned effect parity on a crossing-road lattice, explicit origin connectivity, and sparse LOD handoff. Source and comment review completed before the dependency checkpoint. The available Git checkpoint is `378a0359f09f06a182496df9bd6d7bb56b9c6bce`, pushed to main.

The review found an endpoint cancellation mismatch at large chart coordinates. The focused regression failed with indexed height 30517578.125 and grid height 15258789.0625. Both paths now use the same segment eligibility bounds; the regression and full checks pass. The comment audit suggested no deletions.

## Live implementation and review

RFL pins the immutable core checkpoint. The temporary GeoGrid/Cultivation borrows end before the owned network installs. Two roads between three existing sites share canonical core grading in main and worker. Landscape batches use climate life, one global lattice, exact normal halos, and a repeated-halo aggregate work limit. The existing unit-eight texture is RG8. CPU vegetation consumes exact seeded counts through deterministic thinning of the existing candidates.

The delegated native and runtime write approvals stalled and were aborted without edits. Their text drafts were integrated and corrected by the parent. Native review found inconsistent JS/Rust site ordering, insufficient conservative building exclusion, and pre-clamp source sampling. Bytewise ordering, conservative grid exclusion plus exact final footprint checks, and matching main/native clamped base sampling correct them. Complete source arrays stay unchanged.

The first long road exhausted the smaller two-site sample budget. The final API bounds the entire network at 128,000 expansions and 16,000,000 sample attempts, divided among its connections. Aggregate points are capped at 32,768. Landscape chooses spacing two, four, or eight with at most 200,000,000 estimated operations, including repeated chunk halos. Each core call remains below its own 20,000,000-operation limit.

Progressive mesh requests compile at most 32 new tiles per body and preserve root/intermediate cache entries. Native selection reverses cube-face depth into core levels. A coarse parent remains until all four immediate children are available. Complete network staging compiles the full required tree before publication.

Final review reproduced a discarded incremental delta leaving worker and GPU resource sets inconsistent. Revision mismatches now rebuild both baselines. Review also reproduced a lost continuation when a pending mesh reply arrived during route planning. Unblocking route work now resumes pending refinement, including cancellation and failure while paused. Focused resource-store checks and real browser fault injection verify both corrections. Old worker callbacks remain rejected.

## Principle decisions

- Model the Domain separated the installed immutable network from the selected follower road and pending publication.
- Boundary Discipline placed dimensions, work budgets, density metadata, and effect construction checks at the Wasm/publication boundary.
- Migrate Callers Then Delete Legacy APIs replaced the pair-only planning and effect adapters with whole-network APIs.
- Make Operations Idempotent kept selection free of terrain publication and required a fresh baseline after rejecting an advanced delta.
- Build the Lever extended the committed Chromium driver with paused refinement, stale delta, cancellation, and full-walk checks.
- Prove It Works required actual Wasm mesh deltas, exact coverage reconstruction, the live WebGL2 actor, inspected screenshots, and served-byte comparison.

## Verification outcome

- Core workspace passed 118 checks, Clippy, formatting, Wasm compilation, and frozen route integrity.
- RFL native checks passed 44 tests in total, including three focused network-boundary checks and two progressive coverage tests.
- The complete terrain Node parity suite passed 36 checks. The focused runtime and resource lifecycle suite passed 28 checks.
- All 460 npm workspace/app/domain/release tests passed, and boundaries checked 16 packages.
- Clippy with warnings denied and formatting on every changed Rust file passed. Full native workspace formatting reports pre-existing differences in untouched coordinate, query, and world-wasm files; those were preserved.
- The real Radeon Vulkan browser reported WebGL 2.0 with WebGPU disabled. It installed two roads with 796 total points, followed the selected 203-point road to arrival, switched both roads without worker messages, compared visible material pixels, reproduced planning, rejected a delayed stage, resumed cancelled paused refinement, and rebuilt after a stale progressive delta.
- Every reconstructed native/Wasm publication covered all six cube faces without parent/child overlap. The real browser observed pending and coarse fallback tiles, then settled with zero missing coverage.
- The midway screenshot was inspected and shows the road beneath the grounded actor. Generated artifacts remain ignored in RFL `.workspace-build/proofs/ground-routes/`, including `report.json`, `mid-road.png`, and `tailscale.json`.
- Tailscale served all eleven checked runtime scripts and Wasm artifacts byte for byte, with the new network controls present. No Tailscale configuration changed.

The source audits used GPT-family agents and do not constitute independent cross-family judging. Comment review found no new suppression or constraint comments. The core dependency checkpoint is committed and pushed. RFL network and LOD checkpoint `fddb4e396fe8ac8e29edb328600d31b6312a8e93` and gas-visibility follow-up `17d511745f911b61a8c2c3de5a0b61430db9e4c3` are committed and pushed to `plant-geometry-lod-workbench`. Unrelated ongoing RFL Interiors/rendering changes were preserved.
