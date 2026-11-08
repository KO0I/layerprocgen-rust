# C++ to Rust and WebGL migration

## Goal and completion checks

Port the implemented C++ baseline `8fb913c7667e576b8f8cdbb62295d2e2a222b46b` into Rust. Run its deterministic points sample in a browser through WebGL2. The existing C++ terrain sample is deferred upstream and is outside behavioral parity.

Completion requires deterministic vectors matching executable C++, meaningful lifecycle and save-state tests, offline native and wasm builds, and a browser check that verifies WebGL2 rendering, movement, resource retirement, and shutdown.

## Work list

- [x] Read the Principles section of poteto-mode.
- [x] Phase A: Frame.
- [x] Phase B: Design the workflow.
- [x] Phase C: Run the loop.
- [x] Phase D: Keep the audit trail.
- [x] Phase E: Verify and hand back.

## Workflow

1. Ground the implemented C++ common, core, samples, and persistence contracts. Compare two Rust architecture candidates.
2. Port common numeric types and deterministic hashing. Verify C++ vectors and edge arithmetic.
3. Port the synchronous retained-request runtime, samples, utilities, and save state in disjoint modules. Verify dependency rollback, movement, repeated requests, pooling, queue ordering, and shutdown.
4. Build a browser viewer using Rust WebAssembly and direct WebGL2. Exercise real input and inspect rendered pixels.
5. Run the whole offline test suite, lint, formatting, native example, wasm build, source comparison, and independent review. Document actual scope and limits.

## Throughput checkpoint

- Blocking first steps. Confirm the source baseline, local model health, Rust wasm target, and browser tooling before implementation.
- Independent workstreams. Common types, core runtime, persistence, and the viewer have separate file ownership. Integrate through agreed domain types.
- Shared mutable state. The parent owns Cargo.toml, src/lib.rs, workflow files, and integration. Delegates drafted disjoint modules and tests. Their write approvals stalled, so the parent reviewed and applied the drafts through the main session. Local inference is serialized on the existing GPU backend.
- Smallest safe decomposition. Use one delegate per domain. Keep scheduling synchronous for browser support and deterministic parity. The parent performs integration and verification.

## Design selection

Two designs were considered. A single world arena would centralize ownership but defer release commands until a tick. Independent `Rc`-owned typed layers preserve immediate RAII release and fit the C++ sample. The selected core uses the latter with weak request tokens and incarnation-checked chunk handles.

The browser options were a raw Wasm ABI with JavaScript WebGL or Rust bindings through `web-sys`. The selected viewer uses `web-sys` so the renderer remains Rust and the host script only initializes Wasm and forwards input. The core has no third-party dependencies.

Architecture candidates were examined by GPT models. The available model list does not provide a different model family for the cross-family judge requested by the skill. No independent cross-family review is claimed.

## Intentional adaptations

- Request IDs remain monotonic across recursion. A separate executable probe reproduced a duplicate-ID exception in the C++ baseline.
- Numeric overflow is explicit. Point arithmetic wraps; magnitude uses an unsigned widened result and fixed-point rounding avoids overflowing intermediate addition.
- External dependency registrations remain weak. An acquired external lease retains its provider until consumer destruction finishes. Cyclic generation returns an error.
- Chunk results use `Rc<T>`. Reads release record borrows before invoking callbacks, and destroy hooks receive `&T`. A retired payload can outlive its metadata until its last active reader returns.
- Records are actually reused. The generic object pool resets objects before returning them to its inactive list.
- The scene queue and live bindings are local to one thread. Native executors separately accept owned `Send` work. The port does not expose the C++ compatibility API for a single pending root; `SampleWorld::move_to` owns its replacement request directly.
- The browser uses WebGL2 and GLSL ES 3.00 to match the target game. It preserves the viewer during a persisted `pagehide` event.
- Native save replacement includes a sync before rename. The Windows replacement adapter is present but has not been executed on Windows in this run.

## Verification

The current run passes 58 Rust behavior tests, the headless sample, formatting, workspace Clippy with warnings denied, and an offline WebAssembly release build. The executable C++ comparison matches 759 rows. Browser checks cover rendered pixels, input, streaming, return to origin, shutdown and restart, mobile resize, and persisted page lifetime with WebGPU unavailable.

The final WebGL2 viewer passed on SwiftShader and the Radeon 8060S through ANGLE Vulkan. The GPU check rejects software fallback. Final GPU evidence is recorded in `target/browser/report.json`; browser artifacts are generated rather than committed.

A separate GPT-6 Astra source review found callback reentry, origin deregistration, nested executor, eager request allocation, and page-lifetime issues. Focused regression checks cover the accepted fixes. Destruction-hook test violations are asserted outside hooks so swallowed callback panics cannot hide failures. Cross-family review was unavailable. The bundled control UI skill was unavailable, so a committed Chromium DevTools script drives the real browser.

## Attention

Reviewed by `gpt-6-astra` through source review. Accepted findings were corrected and checked by the parent. No different-family Codex judge was available. The final local SuperGemma documentation check used the updated Hermes worker helper with a four-turn budget and reported no contradictions between the README and port plan.

Windows file replacement remains untested on Windows. Climate import, terrain algorithms, and path generation remain application work. The decision trail was checked against the source and this conversation's tool results; no on-disk transcript path was provided.
