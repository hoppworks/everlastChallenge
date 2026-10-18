# P06 central launcher: reviewed integration

Prospective developer tooling only. The current pretranscription candidate
failed its quality gate; no performance launch, freeze or production activation
is authorized by this integration.

The three new files were imported from
`artifacts/p06-pretranscription-screening/p06-central-pairs-74ffe2d-r2.tar.gz`
(SHA256 `e93344f23a9493173474b24dc480a32aad19a64c99ead689be0f2b5d49869236`).
The archived remainder/screening dependency copies were **not** imported.
The remainder adapter retains its separately committed integration `0cc8753`.
The combined independent remote review and its targeted r2 closure precede this
integration; the frozen r2 archive remains unchanged.

The launcher accepts only complete reference→candidate or candidate→reference
pairs, each block one process with one warmup and six times30 measured slots.
Separate pair windows are allowed, partial block resume/retry is not. It binds
an externally accepted quality decision and a known public numerical report;
red/missing quality or invalid raw bindings prevent comparison/continuation.
It preserves362 slots on failure. No runtime/decoder, measurement budget,
manifest or quality gate was modified.

## Actual Mac compatibility regression

Python3.9.6/Unicode13.0.0 initially produced **one error out of nine**:
`test_quality_decision_is_bound_to_complete_screen_and_this_candidate`
rejected a temporary-directory root spelled `/var/...` because its resolved
child was spelled `/private/var/...`.

The only source delta from reviewed r2 is one line at the quality-decision
boundary: normalize the caller-selected root with `Path(root).resolve()`, as
preflight already does. The neutral path containment and symlink policies are
retained. No test expectation was weakened. The same test then passed.

## Verification

- `/usr/bin/python3 -m unittest discover -s tests/quality -p test_pretranscription_central.py`:
  nine passed (0.312s), including complete181-row modes, twelve corruptions,
  raw-binding error propagation, contradictory numerical reports, separate
  windows/no resume/lock and exact argv.
- `... -p test_paced_remainder.py`:21 passed (0.324s), including the actual
  owned Mac `/usr/bin/time -l` process-group regression.
- `... -p test_pretranscription_screening.py`:17 passed (0.806s), checking the
  shared validator/owned-process seams. These synthetic/control subprocesses
  perform no model inference or OS scheduling-policy change.
- Python3.9 `py_compile` and `git diff --check`: exit0.
- `node scripts/guardrails.mjs`, `node scripts/check-contracts.mjs` and
  `node scripts/secrets.mjs`: exit0; no secret finding.

Final launcher SHA256:
`def627a92913fe29554d9e91fcd8249c30898cbcebce84626109c90246721625`.
The tests and schema are byte-identical to reviewed r2, SHA256 respectively
`09a7bc12b862b1c0ada0e22ce41c1d508001ed80b23c752de52caa79c799c6d4`
and `6a069855a969c93bd5b39abfbeba956f85f750caf02f19a8e5663fff1c4a0126`.

No real pair, model, ASR, audio replay, application, bundle or release build was
run. No actual hypothesis/audio transfer occurred. No P05/P06/native acceptance
or general latency/resource improvement is claimed.
