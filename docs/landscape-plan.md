# Landscape material and vegetation work record

## Goal and scope

Port LandscapeLayer material classification and detail density on top of composed GeoGrid terrain. Adjacent patches must share identical normals and weights, and overlapping cell requests must reproduce density counts. Climate colors stay with the caller. Roads, openings, and reserved geometry exclude vegetation. Underground cover and final heights stay authoritative.

An optional scope question was presented before implementation about continuing into live multi-road integration and LOD. Without a reply, this run implements and verifies the recommended material and vegetation checkpoint. The existing RFL multi-road and Plate/Orbital integration boundaries remain explicit.

## Work list

- [x] Read the poteto-mode principles and ground the C# rules.
- [x] Compare owned-halo and source-trait designs.
- [x] Add conservative road footprint metadata and verify thin-road exclusion.
- [x] Implement halo normals, material weights, and globally seeded density.
- [x] Verify adjacent patches, underground protection, input validation, and budgets.
- [x] Run workspace tests, Clippy, formatting, Wasm checks, and review.
- [x] Record final evidence and hand back.

## Throughput checkpoint

The risky dependency is the halo. Reusing patch-local neighbors creates material seams. Vertex-only road masks also miss roads narrower than a grid cell. GeoGrid therefore owns road/cell intersection metadata, and Landscape owns the one-vertex halo contract and derivation. No renderer, climate generator, or Interiors geometry changes are needed for this CPU checkpoint.

Candidate A accepts an owned immutable GeoGrid halo. Candidate B fetches through a source trait. The chosen base is A because it exposes the exact required bounds without adding a second source interface and gives validation a single boundary. B contributes explicit work accounting for cell/road intersection. The request constructs its halo with checked global indices; build rejects a mismatched halo. Cultivation demand must cover that halo until the consumer unloads it.

The verification units are GeoGrid road-cell metadata, the Landscape rules, and then split/combined patch behavior with existing roads and protected geometry. One bounded local reader fact-checks the staged C# excerpt. Codex owns integration and verifies every accepted fact. No shared writers or additional GPU services are introduced.

## Source rules and adaptations

The source uses `normal.y < 0.65` for cliffs, removes path material there, and fills remaining weight with terrain or cliff material. Detail counts require a four-corner average above 0.4, then use rounded maximum-corner weight times 10 plus jitter. The source queues null tree instances.

The Rust normal uses physical upward height-field orientation, with negative X and Z height derivatives. This changes the source's positive lateral signs while preserving its Y-based cliff test. Source patch-local randomness becomes global signed 64-bit cell addressing with a full world seed. Climate plantability is independent from the climate base-color channel. Final heights and underground-cover metadata are not lowered by the source's Unity edge-skirt pass.

The configured architecture candidates are GPT-family models. No independent cross-family verdict is claimed.

## Implemented result and review

`src/landscape.rs` derives immutable output from an exact GeoGrid halo. Nine Landscape tests and two additional GeoGrid cases cover the checkpoint. The final split/combined fixture includes two crossing roads, a buried volume, and an opening touching both sides of the shared border. Shared vertex and cell comparisons remain exact. Only the ideal analytical road-center coverage assertion uses a small floating-point tolerance.

The local GPT-OSS worker completed its four-turn factual source task. Its quoted normal, cliff test, detail threshold, and density expression were checked against the actual staged C# excerpt before acceptance. No implementation or review verdict was delegated to that worker. It reused the existing backend and GPU without model changes.

The first correctness review reproduced an ownership error. A caller could change positive GeoGrid spacing after construction and silently reclassify a cliff using old geometry and road exclusions. The focused check failed in `.audit/landscape/spacing-before.log`. GeoGrid spacing, vertex buffers, and cell topology are now private with read-only accessors. Three compile-fail checks forbid independent edits, and known callers use the new read-only interface. Landscape no longer repeats validation of those internally validated immutable buffers.

The no-comments pass found no flags. It deleted zero comments, restored none, and found no scoped suppressions or open constraint comments. Copyright, source provenance, and the public halo and lifetime contracts remain.

## Verification

The full workspace passes 109 checks, consisting of 106 runtime tests and three compile-fail API checks. Workspace Clippy with warnings denied, formatting, the workspace Wasm target check, and frozen RFL route fixture integrity all pass. The fixed RFL elevation export also supplies a Landscape seam and unchanged-height test. Plantability in that real-terrain case is controlled test input, not live climate vegetation consumption.

Actual evidence is recorded in `.audit/landscape/workspace-tests.log`, `checkpoint-tests.log`, `clippy.log`, `wasm-check.log`, and `fixture-check.log`. The root inspected the actual source and final outputs rather than accepting worker reports. No on-disk transcript path was provided, so the trail was checked against this conversation's tool output and those files.

## Attention

Reviewed by `gpt-6-astra`. The final source and evidence audit found no blocker. It confirmed the immutable spacing fix, cell road exclusion, request bounds, and documented scope. The root then strengthened and reran the combined intersection/opening checkpoint and full native checks. `gpt-6.1-terra` performed the earlier correctness review and `gpt-6.1-sol` the comment pass. These are GPT-family reviews, not independent cross-family judging.

The complete composition snapshot, stable plantability callback, and halo-wide cultivation demand remain caller obligations. Live RFL multi-road publication, plant/render consumption, and LOD coverage selection remain subsequent work. No scope reply arrived during this checkpoint. Climate generation, RFL WebGL2, and Interiors geometry were not edited. Changes remain uncommitted.
