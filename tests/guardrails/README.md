# Regel-Fixtures

`node scripts/prove-guardrails.mjs` erzeugt isolierte, nicht ausgelieferte Rust- und TypeScript-Fixtures. Ein gültiger Kontrollfall muss bestehen; absichtliche Verstöße müssen mit nicht-null Exitcode scheitern. Die generierten Dateien liegen ausschließlich unter `.generated/` und werden nicht als Produktcode ausgeliefert.

Der Coverage-Negativnachweis instrumentiert ein echtes Rust-Testbinary mit rustc/LLVM und führt einen Vitest-V8-Lauf mit ungetestetem Kontrollzweig aus. Das prüft die tatsächlich gemessene Abdeckung und den fehlschlagenden Grenzwert. Produkt-Coverage ist vor der ersten ausführbaren Produktlogik ausdrücklich nicht anwendbar.
