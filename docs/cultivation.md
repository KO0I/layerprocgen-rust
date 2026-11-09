# Retained cultivation paths

`cultivation::CultivationLayer` adapts the C# sample's connection-to-path generation and overlapping-path queries. It uses the existing callback-based Rust planner. It retains multiple immutable CPU roads independently of rendering.

## Use

1. Supply existing location records, their `LocationConnections`, path options, and a terrain sampler to `CultivationLayer::new`.
2. Call `retain_patch(bounds)` when a terrain patch loads. Keep the returned `CultivationPatch` with that patch.
3. Call `roads_overlapping(bounds)` for the retained roads whose full-width bounding boxes touch a terrain patch.
4. Drop the lease when the patch unloads. A road leaves the cache when its last patch lease is dropped.

Positions must share one planar chart. For RFL, resolve building approach points before constructing the layer. The layer owns one fixed terrain revision, connection set, and path configuration. Rebuild it after base terrain or connection changes. The sampler must describe the same immutable terrain for the layer's lifetime. Dropping the layer before its leases is safe.

A conservative envelope around each connection includes the search corridor, surface, shoulders, and material width. A patch acquires all candidate roads whose envelopes touch it, even if a completed road takes a different part of that corridor. This avoids losing roads with owners outside the queried patch. Exact queries then filter by completed full-width road bounds, matching the source's bounding-box query semantics. Queries alone do not create demand. Tangential boundary contact is included.

Overlapping patch leases share the same `Rc<Road>` and do not repeat planning. A request stages all missing paths before adding demand. If any path fails, no new roads or leases are published, and existing roads remain valid. The error identifies the failed connection. Releasing the last lease removes cache membership immediately. Callers may keep an `Rc<Road>` after release; its immutable payload stays valid but no longer occupies the retained collection. Revisiting plans against the same snapshot again.

The source's GeoGrid height and location-distance cost arrays become the caller's terrain sample callback. The source's generated locations, endpoint rounding and radius-based corrections are replaced by caller-resolved approach positions. Widths remain explicit options. No Unity terrain objects, GPU buffers, RFL worker messages, junction grading, persistence, or live multi-road installation are introduced by this module.

The collection uses deterministic endpoint-ID order. Every individual search obeys the existing planner's expansion and sample budgets. The adapter remains synchronous; callers should bound patch and connection counts and run large requests in their terrain worker.

## Verification

`cargo test --locked --offline --test cultivation` exercises multiple roads, shared demand without repeat sampling, cross-owner material-width overlap, retirement, deterministic regeneration, failure rollback, invalid inputs, distant patches without sampling, and leases outliving the layer. The full workspace tests cover the planner and underlying generation core.

The adapted C# source is `Samples/TerrainSample/Scripts/Generation/Layers/CultivationLayer.cs`, copyright Rune Skovbo Johansen, under MPL-2.0.
