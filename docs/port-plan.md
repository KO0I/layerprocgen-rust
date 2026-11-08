# Rust port plan

## Reference baseline

Use the original C# source as the behavioral authority and C++ commit `8fb913c` as a tested implementation reference. C++ has 79 passing tests at repository creation. Its deterministic vectors were derived from source, not confirmed by executing C#/Unity.

## Implementation sequence

1. Port coordinate arithmetic, half-open bounds, fixed-point values, and RandomHash. Test negative coordinates and explicit wrapping arithmetic against reference vectors.
2. Build a deterministic synchronous chunk runtime with retained requests, dependency acquisition, release, pooling, and rollback on failure. Port lifecycle behavior tests before adding concurrency.
3. Port the headless PointsLayer and PointsSpawningLayer sample. Verify overlapping requests, movement, reuse, and shutdown.
4. Add a separate raylib viewer that displays the headless sample. Keep GPU handles and raylib calls on the rendering thread and keep rendering dependencies outside the core crate.
5. Port terrain noise, geo grid, locations, cultivation, paths, and terrain deformation. Capture executable C# reference fixtures when a reference runtime becomes available.
6. Add terrain meshes, camera controls, streaming, floating origin, and LOD. Check boundary continuity and resource retirement.
7. Add bounded background generation with explicit cancellation and shutdown, then save-state behavior, performance measurements, and packaging.

## Ownership direction

Prefer explicit chunk IDs and owned generation results over a direct translation of C# object references or C++ callback graphs. Choose concrete types as each behavior is implemented. Workers produce CPU data; the rendering thread owns GPU resources. Begin synchronously so deterministic behavior can be checked independently of scheduling.

## Completion criteria

A raylib terrain viewer streams across chunk boundaries and origin shifts without seams, retires resources safely, and shuts down cleanly. Fixed-seed fixtures cover heights, paths, and lifecycle behavior. Build instructions, asset licenses, controls, performance measurements, and intentional differences from Unity are documented.
