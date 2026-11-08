#!/usr/bin/env python3
"""Build the Rust WebGL viewer without a JavaScript package manager."""
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
os.chdir(root)
subprocess.run(["cargo", "build", "--locked", "--offline", "--release", "--target", "wasm32-unknown-unknown", "-p", "layerprocgen-web"], check=True)
subprocess.run(["wasm-bindgen", "--target", "web", "--out-dir", "web/pkg", "--out-name", "layerprocgen_web", "target/wasm32-unknown-unknown/release/layerprocgen_web.wasm"], check=True)
print("Viewer built. Serve with python3 -m http.server 4173 --bind 127.0.0.1 --directory web")
