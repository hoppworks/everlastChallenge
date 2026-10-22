
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
