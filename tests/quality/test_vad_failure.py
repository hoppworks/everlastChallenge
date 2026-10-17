"""Model-free contract test of the actual vendored public VAD function.

Baseline HEAD 3583e83 is expected RED for compute/alloc failures. This does not
load weights, execute GGML, or establish VAD accuracy. Only backend boundaries
are doubles; whisper_vad_detect_speech is extracted verbatim on every run.
Run: /usr/bin/python3 -m unittest discover -s tests/quality -p test_vad_failure.py -v
"""
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp"


def extract_function(source):
    """Use unique public-function boundaries, retaining every interior byte.

    The following public signature is an independent closing boundary; changes
    to either boundary fail explicitly rather than compiling an accidental slice.
    No source rewriting, condition substitution, or copied algorithm is used.
    """
    start = list(re.finditer(r"^bool whisper_vad_detect_speech\(\n", source, re.M))
    stop = list(re.finditer(r"^int whisper_vad_segments_n_segments\(", source, re.M))
    if len(start) != 1 or len(stop) != 1 or stop[0].start() <= start[0].start():
        raise ValueError("unique bounded VAD function region not found")
    function = source[start[0].start():stop[0].start()]
    if not function.endswith("}\n\n") or len(function.encode("utf-8")) > 16384:
        raise ValueError("unexpected VAD function boundary/size")
    return function


BOUNDARIES = r'''
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>
#include <vector>
#define WHISPER_LOG_INFO(...) ((void)0)
#define WHISPER_LOG_ERROR(...) ((void)0)
#define WHISPER_LOG_DEBUG(...) ((void)0)

struct ggml_tensor { std::vector<float> data; };
struct ggml_cgraph { ggml_tensor frame{{0,0,0,0}}, prob{{0}}; };
struct Scheduler {
    bool dirty = false;
    bool fail_alloc = false;
    int fail_frame = -1;
    int computes = 0;
    int resets = 0;
    std::vector<float> outputs;
};
struct whisper_vad_context {
    int n_window = 4;
    int n_threads = 1;
    int buffer_clears = 0;
    int * buffer = &buffer_clears;
    std::vector<float> probs;
    struct { Scheduler * sched; } sched;
    int64_t t_vad_us = 0;
    ggml_cgraph graph;
};
static void ggml_backend_buffer_clear(int * buffer, int value) {
    if (value != 0) std::abort();
    ++*buffer;
}
static ggml_cgraph * whisper_vad_build_graph(whisper_vad_context & ctx) { return &ctx.graph; }
static bool ggml_backend_sched_alloc_graph(Scheduler * s, ggml_cgraph *) {
    // An allocation attempt may leave resources behind even on failure. A
    // second allocation cannot succeed until the owner actually resets them.
    if (s->dirty) return false;
    s->dirty = true;
    return !s->fail_alloc;
}
static ggml_tensor * ggml_graph_get_tensor(ggml_cgraph * g, const char * name) {
    return std::string(name) == "frame" ? &g->frame : &g->prob;
}
static int64_t ggml_time_us() { static int64_t tick = 0; return ++tick; }
static int64_t ggml_nelements(ggml_tensor * t) { return t->data.size(); }
static void ggml_backend_tensor_set(ggml_tensor * t, const void * data, size_t offset, size_t bytes) {
    if (offset || bytes != t->data.size() * sizeof(float)) std::abort();
    std::memcpy(t->data.data(), data, bytes);
}
static void ggml_backend_sched_reset(Scheduler * s);
static bool ggml_graph_compute_helper(Scheduler * s, ggml_cgraph * g, int, bool) {
    const int frame = s->computes++;
    if (!s->dirty || frame == s->fail_frame) {
        // Match the real helper: a failed compute resets its scheduler before
        // returning false. Public-function cleanup must remain safe afterwards.
        ggml_backend_sched_reset(s);
        return false;
    }
    if (frame >= static_cast<int>(s->outputs.size())) std::abort();
    g->prob.data[0] = s->outputs[frame];
    return true;
}
static void ggml_backend_tensor_get(ggml_tensor * t, void * data, size_t offset, size_t bytes) {
    if (offset || bytes != sizeof(float)) std::abort();
    std::memcpy(data, t->data.data(), bytes);
}
static void ggml_backend_sched_reset(Scheduler * s) { s->dirty = false; ++s->resets; }
'''

CONTRACT = r'''
static int failures = 0;
static void check(bool condition, const char * message) {
    if (!condition) { ++failures; std::cerr << "FAIL: " << message << '\n'; }
}
static void next_job(Scheduler & s, std::vector<float> values, int frame = -1, bool alloc = false) {
    s.computes = 0;
    s.outputs = values;
    s.fail_frame = frame;
    s.fail_alloc = alloc;
    // Deliberately never resets dirty/resources: only real function may do so.
}
int main(int argc, char ** argv) {
    if (argc != 2) return 2;
    Scheduler scheduler;
    whisper_vad_context context;
    context.sched.sched = &scheduler;
    const float audio[12] = {0.1f,0.2f,0.3f,0.4f,0.2f,0.3f,0.4f,0.5f,0.3f,0.4f,0.5f,0.6f};
    const std::vector<float> previous = {0.91f, 0.82f, 0.73f};
    next_job(scheduler, previous);
    check(whisper_vad_detect_speech(&context, audio, 12), "initial successful public return");
    check(context.probs == previous, "initial complete positive probabilities");
    check(!scheduler.dirty, "initial success scheduler reset");
    const std::string scenario = argv[1];
    if (scenario != "success") {
        if (scenario != "first" && scenario != "middle" && scenario != "allocation") return 2;
        const int before = scheduler.resets;
        next_job(scheduler, {0.21f,0.32f,0.43f}, scenario == "first" ? 0 : scenario == "middle" ? 1 : -1, scenario == "allocation");
        const bool success = whisper_vad_detect_speech(&context, audio, 12);
        check(!success, "failed backend must return public false");
        check(context.probs.empty(), "failure must expose no partial/stale probabilities");
        check(!scheduler.dirty && scheduler.resets > before, "failure must reset scheduler before return");
        if (scenario != "allocation") check(scheduler.computes == (scenario == "first" ? 1 : 2), "failure injection reached requested frame");
    }
    const std::vector<float> recovered = {0.12f,0.34f,0.56f};
    next_job(scheduler, recovered);
    check(whisper_vad_detect_speech(&context, audio, 12), "next successful call must recover");
    check(context.probs == recovered, "next call must expose every new probability");
    check(!scheduler.dirty, "recovered success must reset scheduler");
    check(context.buffer_clears == (scenario == "success" ? 2 : 3), "real calls must clear recurrent state independently");
    return failures ? 1 : 0;
}
'''


class VadFailureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which("clang++") or shutil.which("c++")
        if not compiler:
            raise RuntimeError("local C++ compiler required for actual-source regression")
        if SOURCE.stat().st_size > 2 * 1024 * 1024:
            raise ValueError("unexpected oversized C++ source")
        function = extract_function(SOURCE.read_text(encoding="utf-8"))
        cls.function_sha256 = hashlib.sha256(function.encode("utf-8")).hexdigest()
        cls.temporary = tempfile.TemporaryDirectory(prefix="everlast-vad-contract-")
        cls.addClassCleanup(cls.temporary.cleanup)
        directory = Path(cls.temporary.name)
        source = directory / "vad_contract.cpp"
        source.write_text(BOUNDARIES + "\n" + function + "\n" + CONTRACT, encoding="utf-8")
        cls.executable = directory / "vad_contract"
        process = subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", str(source), "-o", str(cls.executable)], capture_output=True, text=True, timeout=30)
        if process.returncode:
            raise RuntimeError("actual-function harness did not compile:\n" + process.stderr)

    def assert_contract(self, scenario):
        process = subprocess.run([str(self.executable), scenario], capture_output=True, text=True, timeout=5)
        self.assertEqual(process.returncode, 0, "extracted function SHA256=" + self.function_sha256 + "\n" + process.stdout + process.stderr)

    def test_successful_public_call_and_reuse_are_complete(self):
        self.assert_contract("success")

    def test_first_frame_failure_is_false_empty_and_reusable(self):
        self.assert_contract("first")

    def test_middle_frame_failure_is_false_empty_and_reusable(self):
        self.assert_contract("middle")

    def test_graph_allocation_failure_after_success_clears_and_recovers(self):
        self.assert_contract("allocation")


if __name__ == "__main__":
    unittest.main()
