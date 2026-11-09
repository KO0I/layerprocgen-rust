# GeoGrid composition work record

## Goal

Compose location and road effects on the existing base terrain. Adjacent patches must produce identical samples at shared global grid coordinates. Underground rooms, hulls, and intentional openings need explicit protection instead of a surface-only flattening rule.

## Work list

- [x] Read the Principles section of poteto-mode.
- [x] Ground the C# composition and RFL structure conventions.
- [x] Compare two type and composition sketches.
- [x] Implement global-coordinate composition and verify overlap behavior.
- [x] Integrate retained cultivation roads and protected structure constraints.
- [x] Verify adjacent patches, negative coordinates, effect order, openings, and roof cover.
- [x] Record final checks and hand back.

## Throughput checkpoint

The risks are inconsistent contributor sets at patch borders, order-dependent grading at road intersections, and treating underground rooms as flat surface pads. The bounded implementation belongs in the dependency-free Rust CPU crate. Current RFL live roads remain climate-only and single-road, so this work must not pretend to install a plate or Orbital road network.

The parent owns implementation and review. A local Hermes reader checks the C# formulas with five tool turns. A Codex reader traces RFL room and surface geometry without editing. Two read-only architecture candidates compare immutable effect snapshots against per-patch composition. No shared writers or model switches are needed.

The verification units are pure composition, surface constraints and openings, then the existing cultivation collection with shared global grid samples. The test artifact is a reproducible Rust integration suite. Full Rust tests, formatting, warnings-denied Clippy, and a Wasm target check finish the run. Existing uncommitted location and cultivation work is preserved.

## Grounding and design choice

The C# GeoGrid is the locations-only input to Cultivation. Landscape later applies locations then roads. Heights and influences use normalized eighth-power segment weights. Minimum location metadata stores signed distance to the inner edge and a vector toward the center. The existing RFL material adaptation uses maximum coverage rather than C# summed splats.

Candidate A uses an immutable analytic effect snapshot and global lattice samples. Candidate B caches leased global tiles. The selected base is A because it prevents patch-local contributor selection without introducing a second streaming lifecycle. B contributes the full-cell constraint rule and explicit cell topology. An index or grid cache can be added later without changing composition semantics. The available models are all GPT-family, so no independent cross-family verdict is claimed.

The complete snapshot sorts effects by stable ID. Planning uses locations only. A final snapshot adds all retained cultivation roads, never a different patch-local subset. Locations compose before roads; roads within that stage blend together. Coordinates come from global integer lattice indices. Effect order and patch load order cannot change a vertex's result.

RFL buried prisms provide roof plus cover as a minimum surface height. A conservative rectangular footprint can overprotect irregular rooms. For grid reconstruction, constraints extend to all vertices of every touched cell. Reservations preserve source terrain around shafts, hull extensions, and open-to-sky special geometry. Openings carry cell topology and prevent ordinary terrain walking. Existing base cover violations are errors, not requests to repair climate generation. Room floors never become flattening targets.

The source explorer confirmed local Interiors metres and Y-up, Plate normalization by 28,000 metres in the flight geometry, and live Orbital inward-facing cylindrical coordinates. Adapting those frames into this planar chart is explicit future live integration. Runtime ship collision stays authoritative.

The bounded local GPT-OSS reader exited with an unreliable report after failing to read the actual blend kernel. Its summed-height, linear-weight and underground claims were rejected against PathsBurstMethod.cs. No implementation uses that report.

## Implemented contract and review

The implemented module is `src/geogrid.rs`. `GeoGridLayer` owns the locations-only snapshot and base sampler. `GeoGridComposition` captures complete canonical road geometry. `compose_patch` acquires cultivation demand, generates final terrain, and returns a patch that owns its lease. Road and patch lifetime remain separate from renderer resources. The public reference is `docs/geogrid.md`.

The source review found three actionable details. Road point counts now validate before copying. Reservation-intersection work now has a checked total budget. The fractional-spacing cover support originally used floating-point addition while cell bounds used global index multiplication. A focused check failed with an unprotected height of -40 instead of the roof minimum 12 at spacing 0.3. Building support from neighboring global grid indices corrected it. The failing and passing checks are recorded under `.audit/geogrid/` and the regression remains in `tests/geogrid.rs`.

The reviewer also identified material coverage outside the grading radius. This is an intentional RFL adaptation and now has an explicit behavior check and reference documentation, alongside maximum mask union. The source's all-segment height weighting is preserved, including its dependence on road tessellation.

Open-to-sky geometry is classified by its actual representation. The Plate canyon has an authoritative floor mesh and is reserved, not automatically treated as an opening. Whole-cell opening topology remains conservative until RFL's exact clipping adapter consumes it. Shared cached tiles were not added.

The no-comments pass reported no flags. It deleted zero comments, restored none, required no reruns, offered no encodings, and found no scoped suppressions or unenforced constraint comments. MPL copyright and public API contracts remain.

## Verification

The full workspace passes 95 Rust tests, including 15 GeoGrid checks. Warnings-denied workspace Clippy, formatting, and the entire workspace Wasm target check pass. The frozen Plate and Orbital prism fixture reproduces from the current RFL generator using `node tools/export-rfl-geogrid-fixture.mjs --check`. Fixtures record actual source geometry and conservative terrain minima, not a live game integration claim.

Existing location and cultivation changes remain uncommitted with this port. RFL climate generation, its WebGL2 renderer, and its live single-road implementation were not edited. The current task's trail was checked against actual source and tool output in this conversation; no on-disk transcript path was provided. All configured Codex models are GPT-family, so the architecture selection and code review do not constitute independent cross-family judging.

## Attention

Reviewed by `gpt-6-astra`. The final read-only audit found no blocking defect. It confirmed lease cleanup, canonical composition, the corrected global neighboring support, and the verification logs. The final work-list checkbox is now complete. The no-comments pass used `gpt-6.1-sol`; the earlier correctness review used `gpt-6.1-terra`.

Remaining integration work is explicit. Live Plate and Orbital coordinate adapters and terrain mesh publication are separate. Consumers must honor conservative opening topology and rectangular reservations. Complete graph inputs and an immutable base sampler remain caller obligations. No independent cross-family review was available.
