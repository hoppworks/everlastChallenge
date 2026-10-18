//! Fixed, isolated confirmed-pause experiment; no decoder configuration knobs.
use serde_json::{Value, json};

pub fn decorate(mut value: Value, candidate: bool) -> Value {
    value["configurationId"] = json!(if candidate {
        "p06-pretranscription-candidate-v1"
    } else {
        "p06-pretranscription-reference-v1"
    });
    value["pretranscription"] = json!({
        "candidate":candidate,"productionDefaultChanged":false,"input":"PCM16 mono16000Hz; complete immutable fixture",
        "windowFrames":320,"startRms":0.00005,"endRms":0.000025,"prerollFrames":2400,
        "tailFrames":3200,"confirmationFrames":8000,"cut":"last voice window end+3200, eligible after+8000 stable frames",
        "noForcedCuts":true,"noMinimumUtteranceFilter":true,"noOverlap":true,
        "continuousDsp":true,"flush":"exactly once at recording end",
        "join":"one ASCII space between trimmed nonempty parts; no dedup or repair; total<=256KiB",
        "storage":"at most one frozen journal and one open/final-rest journal; suffix-only copy; actual worker PCM release precedes completion and next permit",
        "initialization":"both arms use identical InferenceService::load serial-worker Engine initialization, one warm context pair, same bounded observer",
        "decoderPolicy":"unchanged full reference; audio_ctx selector0/model1500; flash attentionfalse; native timestamps enabled",
        "maximumJobs":601,"metadataContainsTextOrPcm":false,
        "workerEdges":"before Decoder::decode observer and after its return; not native kernel/full entry; Engine can return at energy/VAD guard",
        "inputProof":"host-submit timestamp compared with complete publication after_publish_nanos; prefix requires timeline.end+4800 published frames, final job timeline.end; missing evidence invalidates run",
        "finalText":"only after audio release/drain/flush, complete disjoint result coverage and final adoption; early results never establish L",
        "lateCleanup":"one retired service-accepted unfinished job record; before-decode may be absent, identity bound with its own capture t0; detached evidence handle owns no Runtime/PCM; exported after cleanup join",
        "qualification":false
    });
    value["preprocessing"]["mode"] = json!(if candidate {
        "continuous stable DSP; confirmed-pause prefixes and one final rest; per-job unchanged reference energy/VAD/ASR preparation"
    } else {
        "matched full-recording reference; unchanged energy/VAD/ASR preparation"
    });
    value["measurement"]["variant"] = json!(if candidate {
        "confirmed-pause-pretranscription"
    } else {
        "matched-full-recording-reference"
    });
    value["measurement"]["mode"] = json!("paced-runtime-pretranscription-experiment");
    value
}
