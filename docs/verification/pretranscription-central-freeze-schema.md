# Prospective P06 central paired launcher — off-host preparation

Developer tooling only. The current candidate failed the quality gate and must
not receive a positive quality decision or be launched for qualification.

## Fixed execution unit

The existing paced_reference_probe runs one warm model/context process, exactly
one labelled DE5 warmup and six canonical cases with30 measured runs each:
181 slots per block. No Engine/Runtime/decoder/pacing change is introduced.

Two independently scheduled, indivisible pair roles:
- ref-cand: reference then candidate;
- cand-ref: candidate then reference.

Each pair may have a separate time window. No fragments, resumed slots, retries
or adopted measurements. A failed first block prevents the second, retaining
both181-slot projections. Compare each pair separately; never average ABBA L.
Complete pairs report relative10% improvement and the candidate's six absolute
median/p95 limits. Full qualification still needs separate quality, resource,
cancel and maximum-duration evidence. Repeats add no lexical coverage.

## Freeze JSON

Required: schemaVersion1, kind prospective-pretranscription-central-pairs-v1,
sourceRevision (full immutable Git commit), and these bindings:

- binary: target/release/examples/paced_reference_probe;
- parameters: object with reference and candidate bindings for the exact
  printed CENTRAL templates (--print-pretrans-reference-parameters and
  --print-pretrans-candidate-parameters), not single-case templates;
- manifest: unchanged central six-case manifest;
- inputs: exactly six audio bindings matching that manifest;
- model and vad: known pinned ASR/VAD hashes/bytes, model id, fixed sibling names;
- sources: unique bindings for every recursive compiled parameter source pin,
  this script/test/schema and reused scripts/run-paced-remainder.py and
  scripts/run-pretranscription-screening.py. Additional sources are allowed
  and checked identically; no generic transitive-closure claim;
- qualityDecision: binding described below;
- outputDirectory: root strictly below actual ignored artifacts/.

Each binding: path, bytes, sha256, mode (actual oct(stat.S_IMODE(...)) string).
Paths are bounded repository-relative paths without traversal. Reads reject
final symlinks/nonregular files. Sources must equal both current bytes and the
immutable revision. Compiled source hashes must agree. Before each launch all
preflight fingerprints are rechecked; ordinary mutation detection, not hostile
same-user concurrent-writer defence.

The parameter pair may differ only in configurationId,
pretranscription.candidate, measurement.variant and preprocessing.mode labels.
All Runtime/FullParams/models/publisher settings remain equal. Required:
audio_ctx0, no_timestampsfalse, suppress_nstfalse, flash_attnfalse; TC publisher;
1warmup/30percase/181total,20ms chunks,10ms jitter,1ms maximum sleep,250us spin.
The binary itself checks its entire compiled parameter document before model IO.

## Bound external quality prerequisite

qualityDecision JSON:
- kind: pretranscription-quality-decision-v1;
- passed: true;
- productionActivated: false;
- screeningFreeze, screeningCompletion, qualityReport: bindings;
- runtime: exact reference Central runtime object;
- candidatePretranscription: exact candidate pretranscription object.

This record is authored only after independent quality acceptance. The launcher
verifies its three evidence bindings and requires completedtrue, four blocks of
100 cases, each caseok with exactly two rows statusok/validatedtrue. It rejects
passedfalse, incomplete/changed evidence and mismatched parameters. It does not
recompute WER/semantics. The bound public qualityReport must use schemaVersion1,
kind pretranscription-quality-screening-decision-v1, decision.numericalScreenPassed
true and decision.candidateRejectedfalse. Missing, unknown or explicitly rejecting
reports fail closed even if the external decision says passedtrue. The public
report must keep productionPretranscriptionEnabledfalse, performanceRunStartedfalse,
qualifiedfalse and Lnull. Overall semantic/anchor approval remains the external
decision.passed authority; no invented semantic-report field is introduced.
Successful inference alone is no quality approval.
The current failed candidate must stay rejected.

## Commands

From repository root:
    python3 scripts/run-pretranscription-central.py FREEZE --pair ref-cand
    python3 scripts/run-pretranscription-central.py FREEZE --pair ref-cand --execute
In another approved time window:
    python3 scripts/run-pretranscription-central.py FREEZE --pair cand-ref
    python3 scripts/run-pretranscription-central.py FREEZE --pair cand-ref --execute

Default is preflight only. Execute requires an explicit pair. The other pair
directory may exist; the selected directory/active-pair lock is never replaced
or resumed. Internally constructed argv (no shell):
    /usr/bin/time -l BINARY MODEL reference MANIFEST VARIANT_PARAMS NEW_RESULT
No run override, reference text, expect-empty or arbitrary command.

Owned process groups/watchdog/kill/reap/group-ended proof reuse the reviewed
screening launcher.7200s is liveness only, not an L threshold. A live owned
idle-sleep assertion and AC snapshot are required before each block, AC checked
again afterward. No continuous power/lid guarantee; keep lid open, power
connected, and all other model work stopped. Wrapper PID and actual metadata
processId are separate evidence.

Pair directory0700; raw result/metadata/log and exclusive atomic publications
0600. Pair completion preserves362 slots, raw bindings, actual process IDs,
statistics and errors. Hypothesis texts remain only in private raw results.
A missing/error/SHA-null raw binding invalidates the block, clears its
validated-row flags/statistics, prevents the next block, and makes pair comparison
null while preserving raw statuses and all362 slots.
File publication is not a multi-file transaction. A separate final
supervisor-completion.json binds pair completion and proves idle assertion
ended; accepting the process archive requires it too.

Each of181 rows requires canonical slot/order/warmup/session; source, manifest,
reference, model, parameter, PID bindings; TC snapshots/owned publisher join;
all publication/stop timings; actual Engine timing; final nonempty text;
contiguous global timeline/local storage agreement; independently reconstructed
published-input proofs; worker settlement/adoption; cross-session monotone jobs;
unchanged postjoin trace; Runtime cleanup join. Independently recompute all180
latencies: median middle-pair mean, p95 position29/30, unweighted six-median L.
Compare with reported statistics; no subset statistics.512MiB metadata and16MiB
separate text bounds.

## Honest preparation status

Public source baseline74ffe2d, remote Python3.14 synthetic checks only.
Nine tests cover both181-row modes,12 corruptions, parameter deltas, complete
quality bindings, separate windows, lock/no-resume and exact argv. Actual REDs
preceded GREEN. No Mac import/build, actual audio/hypothesis transfer, models,
real process measurement, OS policy mutation or ASR run. Remote Python is not
canonical Unicode13 quality evaluation. No qualification/activation claim.
