# Live RFL routes and terrain effects

## Goal and completion checks

Consume generated Rust routes in RFL so an actor travels between existing climate-world locations. Then grade the same route into the terrain and blend a route material mask through the existing WebGL2 renderer. Climate generation and snapshot arrays remain the baseline.

Completion requires live worker planning, body-local following with terrain contact, route cancellation and stale-result rejection, actual changed mesh elevations matching movement queries, visible material coverage, and repeatable browser checks on a climate world.

## Work list

- [x] Read the Principles section of the poteto-mode skill.
- [x] Phase A: Frame.
- [x] Phase B: Design the workflow.
- [x] Phase C: Run the loop.
- [x] Phase D: Keep the audit trail.
- [x] Phase E: Verify and hand back.

## Throughput checkpoint

The risky first step is agreeing on one route record across the retained terrain worker, main-thread elevation queries, and actor movement. Two repositories are involved. The reusable planner is in layerprocgen-rust; the live consumer and terrain effects belong to RFL. No editor or shared rendering package changes are needed.

Grounding is partitioned into worker/movement and terrain/material ownership. Architecture candidates are read-only and return sketches. The parent integrates changes and owns shared files. A bounded local Hermes reader checks copied C# deformation sources inside this workspace; local inference remains serialized on the existing backend.

The units are live route planning and following, pure deformation and mask behavior, terrain/collision/material integration, and end-to-end verification. Each unit ends in a relevant check before the next starts. Existing dirty RFL Interiors and rendering-material work belongs to another task and is preserved.

## Grounding

RFL retains climate SurfaceTerrain objects in its Rust terrain worker. Flight rejects old mesh replies by scene epoch and surface revision. Movement and placement still call the main-thread JavaScript groundElevation function. The route effect must reach both owners. Existing flight autopilot is not a ground follower. RFL already has body-local transforms and a Scout actor bridge.

## Design and verification record

The chosen design uses the existing retained terrain worker and a compact analytic route effect shared by worker and main-thread Wasm. A bounded raster texture represents only material coverage. The seven-float terrain vertex layout and climate textures stay in place. The planner is a pinned Cargo Git dependency at fb0f91e, so release builds need no sibling checkout or copied planner.

Two independent architecture candidates preferred analytic heights over a raster height patch. The base uses the first candidate's dedicated ground follower and mesh-derived building approaches, plus the second candidate's staged terrain installation. Stage a fresh retained surface and resource map, publish geometry and collision effects together, and promote worker state only after acknowledgement. Discarding a packet must not silently advance the worker's delta baseline.

Planning samples the existing base including impacts. New grading therefore applies after those impacts. Any base revision change clears the installed route before re-planning; a road never hides a later crater or grades repeatedly from its own output. This ordering avoids double-applying an existing crater to route target heights.

The first verified unit is live route planning and ground following without grading. The next adds pure deformation and material-mask rules. The last integrates staged terrain publication and browser proof. The canonical decision trail remains docs/decisions.tsv. Configured Codex models do not provide independent model-family judging; no such verdict is claimed.


## Implemented files and checks

The live implementation belongs to the RFL repository at `../rasd-fastwalker-league-js`. The standalone planner remains the pinned public dependency. RFL's `docs/ground-routes.md` documents the controls, runtime API, and reproduction commands.

The fixed live fixture connects the same two existing building IDs through external approaches. Exact projected mesh bounds replace building-center targets. Shared Rust deformation controls mesh and collision heights. Material coverage also excludes plants, and surviving shoulder plants use body-local authoritative queries without cloning away the body's route identity.

The live browser run exercises actual worker planning, staged terrain publication, material pixels, grounded walking through the normal player loop, keyboard brake, jumping, flight navigation, repeatability, and a real delayed stage rejected after a scene change. The initial review found missing X/controller-rise cancellation, conflicting flight navigation, and cloned-body shoulder sampling. Follow-up checks found and corrected transient fall state surviving arrival. A focused handoff test now advances movement timers and verifies ordinary walking mode.

The material mask intentionally unions segment coverage by maximum rather than reproducing C#'s summed splats. A native behavior test makes coverage independent of path sample density. Copyright, the MPL license, and source attribution remain with the adapted module.

The review used GPT-6 Astra for lifecycle and control behavior, GPT-6.1 Terra for native geometry and Wasm boundaries, and GPT-6.1 Sol for comments. Their confirmed findings were corrected. There is no independent model-family verdict. A bounded local GPT-OSS worker read the staged C# sources; its description of weighted splat accumulation was corrected against the actual source before implementation.

Incidental formatter changes outside the route files were removed only after Rustfmt canonical output proved them equivalent. Existing unrelated RFL Interiors and shared-material changes were preserved. The runtime route is bounded and ephemeral; global networks, entrance portals, persistence, and replay synchronization remain outside this slice.

## Verification outcome

Native checks pass 45 tests, including six route-effect behavior checks. The terrain Wasm build and warnings-denied Clippy pass. The 27-check native/browser parity suite passes. All workspace, app, and domain suites pass. The release check initially exposed missing new modules in the explicit allowlist; adding only those entries made all 19 packaging checks pass. The existing unrelated release allowlist addition was preserved.

The real browser report confirms WebGL 2.0 on the Radeon Vulkan backend with WebGPU unavailable. It walks a 219-point live route to arrival, verifies material pixel changes, manual interruption, repeatable replanning, and rejection of a delayed real worker stage. The committed browser driver is the repeatable proof artifact. Its generated report and screenshots remain ignored under RFL `.workspace-build/proofs/ground-routes/`.

Final `npm test` passes all 449 tests after the release allowlist correction. Workspace boundaries and both repository diffs pass their checks. The frozen route fixture still passes its integrity check. The existing Tailscale preview at `https://nepthys.tail1396a4.ts.net:4173/game/?renderBackend=webgl&terrainCompiler=rust` serves the route controls. All thirteen checked route scripts and terrain Wasm assets match the finished local files byte for byte. No Tailscale configuration was changed. The RFL implementation checkpoint is commit `69b73ca` on `plant-geometry-lod-workbench`.
