# Local verification gates

`npm run verify` completed successfully on revision `bce2ae0`. The
[record](local-gates-2026-10-22.json) binds the source tree, lockfiles, toolchain,
macOS version and complete log hash. This is a local execution result, not a
GitHub Actions result.

The initial workspace Rust run reported 579 passing tests and four ignored
tests, including doctests. The frontend run passed 244 tests in seven files;
Node script tests passed 168 with one skipped; tooling tests passed 36. Ignored
and skipped tests are not counted as executed evidence. Repeated execution for
coverage does not increase these counts.

Formatting, Clippy, TypeScript, dependency-use checks, IPC contract checks and
architecture checks passed. Frontend coverage passed at 96.78% of lines. The
host and portable refinement suites passed their configured per-file floors;
these are not whole-application coverage percentages. Guardrail tests exercised
both valid inputs and deliberate failures.

Source scanning, Cargo advisory/license/source checks and npm audit passed. The
additional history scan reported no findings across the history reachable from
this checkout: 423 commits, of which 382 expose content diffs in the default Git
log; the other 41 have no such diff. A scanner result is not proof that arbitrary
private prose or every possible secret is absent. Public documentation was also
reviewed separately for scope and private material.

The dependency check reported an unused historical exception for
`webpki-roots 1.0.10`. The active macOS graph uses the platform certificate
verifier and no longer contains that package. The exception was removed and
`cargo deny check` passed again. This configuration-only cleanup changes no
runtime dependency. Compatible transitive duplicate versions and the documented
maintenance-advisory exception remain visible in the policy and log.

These software checks complement the separate
[packaged observations](refinement-packaged-2026-10-22.md). They do not replace
human linguistic review, acoustic input qualification, real memory-pressure
events or comparative timing measurements.
