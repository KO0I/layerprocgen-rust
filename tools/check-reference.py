#!/usr/bin/env python3
"""Compare Rust output with executable C++ from the recorded baseline."""
import argparse
from pathlib import Path
import subprocess
import difflib

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--cpp", type=Path, default=root.parent / "LayerProCGen-cpp20-raylib" / "cpp")
args = parser.parse_args()
cpp = args.cpp.resolve()
output = root / "target/reference"
output.mkdir(parents=True, exist_ok=True)
subprocess.run(["c++", "-std=c++20", "-O2", "-pthread", "-I"+str(cpp/"include"), str(root/"tools/reference.cpp"), str(cpp/"src/simple_layers.cpp"), str(cpp/"src/save_state.cpp"), "-o", str(output/"cpp-vectors")], check=True)
expected = subprocess.check_output([str(output/"cpp-vectors")], text=True)
actual = subprocess.check_output(["cargo", "run", "--quiet", "--locked", "--offline", "--example", "reference_vectors"], cwd=root, text=True)
(output/"cpp.txt").write_text(expected)
(output/"rust.txt").write_text(actual)
if expected != actual:
    print("".join(difflib.unified_diff(expected.splitlines(True), actual.splitlines(True), fromfile="C++", tofile="Rust")))
    raise SystemExit(1)
print(f"PASS: {len(actual.splitlines())} C++ and Rust numeric, sample, and save-state vectors match.")
