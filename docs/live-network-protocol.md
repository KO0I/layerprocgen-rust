# Live network adapter contract

The next live slice is climate-only. Core protected scalar exports fail explicitly. The complete network publishes once; selection changes only the follower.

## Wasm API

- `planSurfaceNetwork(handle, chart14, sites, siteIdsText, options9, obstacles)` returns `BrowserNetworkPlan`.
- Sites are packed triples `[chartX,chartZ,projectedBuildingRadius]`. `siteIdsText` is newline-separated stable IDs matching those triples. Options retain `[gridSpacing,corridor,sampleSpacing,maxGrade,maxExpansions,maxSamples,surfaceWidth,shoulderWidth,materialWidth]`.
- Plan getters `roadOffsets` and `roadPairs` return u32 arrays. Offsets have E+1 point indices, not float offsets. Pairs have E*2 site indices. `takePoints()` returns all packed `[x,height,z]` points. `approaches` returns N*2 chart coordinates. `free()` releases the plan.
- `new BrowserSurfaceNetwork(chart14, siteIdsText, roadPairs, roadOffsets, points, width3)` owns canonical analytic effects, not climate data. Methods are `applyHeight(x,y,z,source)`, `coverage(x,y,z)`, `elevationBound`, `setPlantDensity(originX,originZ,cellsX,cellsZ,spacing,counts)`, `plantDensity(x,y,z)` and `free()`. Plant density returns -1 outside the atlas and an integer zero through ten inside.
- `setSurfaceNetwork(handle, chart14, siteIdsText, roadPairs, roadOffsets, points, width3)` installs the same owned effects in worker terrain. `clearSurfaceNetwork(handle)` removes it.
- `surfaceNetworkLandscapeSpec(handle)` returns `[originX,originZ,cellsX,cellsZ,spacing]`, where origin values are integer global lattice indices. The worker samples climate life for its entire halo, indices origin-1 through origin+cells+1, into a row-major Float32Array with `(cellsX+3)*(cellsZ+3)` values. Directions use the existing routeDirection.
- `compileNetworkLandscape(handle, plantabilityValues)` returns `BrowserNetworkLandscape`. Getters are `origin`, `cellsX`, `cellsZ`, `spacing`, `width`, `height`, `bounds`. Width/height are cell counts plus one. `takePixels()` returns interleaved two-channel bytes, path then cliff. `takeDensity()` returns exact row-major cell counts. `free()` releases output. Native chunks share one exact global lattice.

## Runtime record

`prepareGroundNetwork` returns schema 2, canonical `siteIds`, matching `siteNames`, common `chart`, packed `sites`, complete `obstacles`, `options`, and shared `width`. At least three sites are required by the UI. Explicit `siteIds` are resolved from ordinary building generation. Default discovery is bounded radius twelve, maximum 768 tiles. The chart uses an orthonormal tangent basis and enforces the existing half-radius bound.

The planned record adds `roadPairs`, `roadOffsets`, aggregate `points`, `approaches`, `sampler`, and `roads`. Each road has a stable `id=JSON.stringify([a,b])`, its two `siteIds`, a human-readable `name`, its own `points` slice, `bodyLocalPoints`, shared chart/obstacles/width. Do not concatenate roads for following. The default selected road is the shortest, with stable ID tie-breaking.

Existing `route-plan`, `route-planned`, `route-stage`, `route-staged`, `route-commit`, and `route-committed` message kinds remain the transaction transport. A record now means a complete network. Request/scene/body/base guards remain unchanged.

`route-staged` includes `mask={width,height,bounds,pixels,channels:2}` and `landscape={origin:[x,z],cellsX,cellsZ,spacing,counts}` in addition to complete terrain resource replacements and eye. The main pending effect receives and validates density before publication. Main publishes materials/mesh/effect/selected record synchronously, then sends commit. Worker promotes only on matching acknowledgement.

The controller retains `plan(body,options)`, `receive`, `start`, `step`, `cancel`, `reset`, and adds `selectRoad(id)`. `getActive()` returns `{body,network,record:selectedRoad,...}`. `getState()` includes `roadCount`, `networkSiteIds`, `selectedRoadId`, selected `siteIds`, selected `pointCount`, width and mask. Selection cancels following, sends no worker messages, and does not replace GPU resources or effects. Cancel stops following while leaving the installed network.

Flight exposes `planGroundNetwork`, `getGroundNetwork`, `selectGroundRoute`, and the existing selected-road visit/walk/cancel/get-state methods. The obsolete pair-only `planGroundRoute` entry is migrated rather than kept as a compatibility layer. The UI says Plan local network and has a labeled road selector.

The material owner uploads RG8 on existing texture unit eight. Shader weights existing climate color by `1-path-cliff`, then path soil and cliff colors. The terrain vertex ABI stays seven floats. CPU plants use exact density counts. A separate stable hash stream keeps an existing non-building plant candidate when `roll % 10 < count`. Outside the atlas, existing placement is unchanged. Native does not choose species or create extra candidates.

Ordinary `compileSurface(handle, eye, detail, maxNewTiles)` requests use a budget of 32. Omitted budget compiles a complete batch for network staging. Delta getters `pendingTiles`, `fallbackTiles`, and `missingCoverage` feed worker diagnostics and the paused continuation. Base sampling includes the existing depth floor before network composition; the final floor remains after it.
