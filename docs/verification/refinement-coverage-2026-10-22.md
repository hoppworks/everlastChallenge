# Explicit refinement coverage scope

The general workspace test run exercised refinement tests, but the previous
coverage commands did not set explicit per-file baselines for the protocol,
worker or extracted host-refinement modules. Workspace membership alone was not
evidence that those files were measured or protected by a coverage check.

`npm run coverage:refinement` now instruments the protocol and worker tests,
including the host supervisor compiled by the process fixtures. Each selected
file must have executable lines and meet its own recorded baseline. A strong
aggregate cannot hide a missing or weaker file. The existing host command also
selects its files directly from its policy, including the extracted controller,
runtime, setup and model-provisioning paths.

New checks reject empty policies, missing files, zero executable-line counts,
invalid thresholds and a below-baseline file beside a stronger file. The initial
test failed before the per-file checker existed, then passed after implementation.

The first instrumented pass exposed sparse automatic coverage of worker asset
handling. Two additional boundary tests now verify that a valid first asset
followed by a corrupt later asset never publishes a partial snapshot, and that
foreign entries, directory aliases and a non-private snapshot parent are
rejected without modifying the source. The valid asset uses an already recorded,
hash-identical model attribution file; no model download is required. These
tests passed without a production-code change. Manifest-module line coverage
increased from 36.02% to 85.31%.

Independent review caught an invalid first version of the snapshot test: the
temporary owner directory could have `0755` permissions and be rejected before
copying any asset. The corrected fixture creates it with `0700` and asserts that
prerequisite as well as the first asset's exact size and hash. A negative control
using `0755` fails that assertion; the restored fixture and updated instrumented
run pass. The earlier measurement is retained as an intermediate result, not
evidence that the snapshot-copy branch was tested.

## Interpretation and limits

The [measurement record](refinement-coverage-2026-10-22.json) lists exact covered
and executable-line counts. `config/coverage.json` records integer floors below
the measured values; they prevent regression and are not a target or a claim of
complete behavior coverage. Host module counts include inline test code, as in
the existing host policy.

The portable command deliberately excludes the MLX model adapter and executable
entry point. Real model loading, GPU inference, termination and bundle behavior
remain supported by their separate native observations, not by these line
percentages. The manifest result does not cover every filesystem failure or
replace native full-model hash/copy tests. The added workflow step is configured
CI behavior, not evidence of a completed remote run.
