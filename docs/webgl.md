# Run the Rust WebGL2 viewer

The viewer renders the ported C++ points sample as cubes. Rust owns generation, chunk retention, scene objects, vertex construction, shaders, and WebGL calls. JavaScript loads WebAssembly and forwards browser input. The core library has no rendering dependencies.

## Requirements

Use Rust 1.88 or later and the `wasm32-unknown-unknown` standard library target. Install `wasm-bindgen-cli` version `0.2.128`, which matches the pinned Rust bindings. The browser needs WebGL2.

The initial dependency fetch requires network access unless the crates are already cached.

```sh
cargo fetch --locked
python3 tools/build-web.py
python3 -m http.server 4173 --bind 127.0.0.1 --directory web
```

Open <http://127.0.0.1:4173>. The server serves the local build only.

## Controls

Drag the canvas or use WASD and arrow keys to move. Pinch with two fingers or scroll to zoom. Twist two fingers to rotate. Slide both fingers up or down to tilt. Home resets the viewing angle. **Home** returns to the original camera position. **Pause queue** holds scene creation and destruction until resumed. **Stop** shuts down chunk requests and destroys the live objects. **Restart** creates a fresh world.

Camera positions remain in double precision until vertices are built relative to the camera. Input is bounded to ten million units in either direction. The world always uses the C++ sample seed `1234`.

## Verify browser behavior

Use Node.js 22 or later and Chromium.

```sh
node tools/check-web.mjs
```

The script downloads no packages. Build the viewer first. The check starts its own local HTTP server and headless browser. It disables WebGPU exposure, checks WebGL2 pixels and errors, drives keyboard and pointer input, tests pause and movement, returns to deterministic source positions, and checks shutdown, restart, and a mobile viewport.

The default browser check uses Chromium's software WebGL2 renderer for repeatability. To exercise the machine's GPU through ANGLE Vulkan, run the same check with `WEBGL_GPU=vulkan`. It rejects software fallback in that mode.

Results and a screenshot are written under `target/browser/`. The browser profile is temporary and separate from your normal browser.

## Compare with executable C++

The parity check compiles the recorded C++ baseline and compares its output with Rust.

```sh
python3 tools/check-reference.py --cpp /path/to/LayerProCGen/cpp
```

The default source path is the sibling `LayerProCGen-cpp20-raylib/cpp` checkout. The outputs under `target/reference/` include hashes, fixed-point values, generated point positions, and the complete encoded save-state fixture.

## Rendering boundary

The viewer uses [`web-sys` bindings for WebGL](https://wasm-bindgen.github.io/wasm-bindgen/examples/webgl.html). Rendering lives in `viewer/src/lib.rs`. Shader compilation and link failures reach the page's error display. The renderer uses one reusable vertex buffer and deletes its GPU resources when the viewer is freed. A lost graphics context displays a reload instruction.

The reference C++ port has no terrain viewer. This browser sample covers its implemented `PointsLayer` and `PointsSpawningLayer`. Unity terrain, path generation, LOD, and persistence UI remain separate work.
