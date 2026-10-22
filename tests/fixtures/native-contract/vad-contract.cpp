
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
