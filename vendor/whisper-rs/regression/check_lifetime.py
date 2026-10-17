#!/usr/bin/env python3
"""Compile actual wrapper modules, no Cargo, model, or native inference."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

VENDOR = Path(__file__).resolve().parents[1]
ROOT = VENDOR.parents[1]
FIXTURE = ROOT / "crates/macos-integration/tests/fixtures/whisper_vad_lifetime.rs"
ORIGINAL_SHA = "d8033831a1b5aff45e8cdb27970bae5d492d1c92d9bf5b5001b91f32fe57be4a"


def run(source, label, temporary):
    binary = temporary / label
    environment = dict(os.environ, WHISPER_VAD_SOURCE=str(source.resolve()))
    subprocess.run(["rustc", "--edition=2021", str(FIXTURE), "-o", str(binary)], env=environment, check=True, timeout=30)
    observed = subprocess.run([str(binary)], capture_output=True, text=True, timeout=10)
    result = json.loads(observed.stdout)
    if result["validPaths"] != 200 or result["nativeDrops"] != 100 or result["unexpectedResults"] != 0:
        raise RuntimeError("constructor seam did not execute expected success/failure paths")
    return observed.returncode, result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original_module", nargs="?", type=Path, help="optional cached original module; otherwise reconstruct the SHA-pinned original from the minimal patch")
    arguments = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="everlast-vad-lifetime-") as temporary:
        original = arguments.original_module
        if original is None:
            # Invert only the published minimal patch, then require exact byte
            # identity with the audited original module. No registry or network
            # dependency is needed on a fresh checkout.
            source = (VENDOR / "src/whisper_vad.rs").read_text()
            patched = '''        let model_path = CString::new(model_path).expect("VAD model path contains null byte");
        let ptr = unsafe {
            whisper_vad_init_from_file_with_params(model_path.as_ptr(), params.into_inner())
        };'''
            baseline = '''        let model_path = CString::new(model_path)
            .expect("VAD model path contains null byte")
            .into_raw() as *const c_char;
        let ptr =
            unsafe { whisper_vad_init_from_file_with_params(model_path, params.into_inner()) };'''
            if source.count(patched) != 1 or source.count("use std::ffi::CString;") != 1:
                raise RuntimeError("minimal audited patch changed; original reconstruction requires re-audit")
            source = source.replace(patched, baseline).replace("use std::ffi::CString;", "use std::ffi::{c_char, CString};")
            original = Path(temporary) / "original_whisper_vad.rs"
            original.write_text(source)
        if hashlib.sha256(original.read_bytes()).hexdigest() != ORIGINAL_SHA:
            raise RuntimeError("original module does not match pinned crate")
        red_code, red = run(original, "original", Path(temporary))
        if red_code != 1 or red["passed"] or red["successFreed"] != 0 or red["failureFreed"] != 0:
            raise RuntimeError("original must fail specifically with both constructor path allocations leaked")
        print("original RED:", json.dumps(red, sort_keys=True))
        green_code, green = run(VENDOR / "src/whisper_vad.rs", "patched", Path(temporary))
        if green_code != 0 or not green["passed"]:
            raise RuntimeError("vendor constructor lifetime regression failed")
        print("vendor GREEN:", json.dumps(green, sort_keys=True))


if __name__ == "__main__":
    main()
