# P06 remainder adapter: reviewed integration

The reviewed developer-only adapter was imported without source changes from
`artifacts/p06-pretranscription-screening/p06-remainder-adapter-74ffe2d.tar.gz`
(SHA256 `6ffd9799dd872aa325d11a4c3f1389fed19501123f27b627c9369957b3a3fda8`).
All three previous files matched immutable `74ffe2d` byte for byte, and the
archived script patch passed `git apply --check` before import.

Only `scripts/run-paced-remainder.py`, its tests and freeze schema changed.
The existing v6 path remains supported. The two explicit pretranscription modes
require all eleven rows, complete trace/input/admission/settlement evidence,
monotone cross-session job identity and unchanged post-join trace. The shared
screening validator must be included in immutable source bindings. No decoder,
model, runtime, production default, parameter gate or corpus was changed.

## Verification

On macOS, `/usr/bin/python3` reports Python3.9.6 and Unicode13.0.0.

- `/usr/bin/python3 -m unittest discover -s tests/quality -p test_paced_remainder.py`:
  **21 tests passed**, including the real owned `/usr/bin/time -l` process-group
  regression proving own-group termination and preservation of a separate
  control group (0.382s).
- `node scripts/guardrails.mjs`, `node scripts/check-contracts.mjs`,
  `node scripts/secrets.mjs`: exit0; no secret finding.
- `git diff --check`: exit0.

The actual remote RED/GREEN evidence and independent review preceded import.
The reviewed script SHA256 is
`38bdc39ba719ffbf53c37e9585224ddb70f6faa8a325089ab1a12d5bf8fcef51`;
tests SHA256 is
`6c1021f345dfe8d4c67d01797ed385a79a37fff04aad19a32c044fa0e73b40d6`.
No Python3.9 compatibility edit was needed.

These are structural/process-safety checks, not an inference or performance run.
The current pretranscription quality rejection remains a start prohibition.
No model, audio, hypothesis, freeze or timing archive was generated or changed;
there is no P05/P06, native app or installed-bundle acceptance claim.
