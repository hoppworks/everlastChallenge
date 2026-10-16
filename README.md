# Everlast Dictation

Native Diktier-App für Apple Silicon und macOS 27. Rust/Tauri 2 mit TypeScript-Oberfläche, lokale Whisper-Inferenz und systemnahe Audioaufnahme/Textübergabe.

**Entwicklungsstand:** Gerüst in Arbeit. Noch kein abgenommenes Diktierprodukt und kein zur Installation freigegebener Build. Fortschritt, tatsächliche Prüfungen und offene Abnahmen stehen im [Ausführungsprotokoll](docs/execution-log.md). Der [Implementierungsplan](IMPLEMENTATION_PLAN.md) enthält den vollständigen Zielumfang.

## Lokal entwickeln

Geprüfte Ausgangsumgebung: macOS 27.0.1, arm64, Rust/Cargo 1.93.0, Node 22.23.1, npm 10.9.8 und Xcode 27.1. Kein anderer Rechner wurde geprüft.

1. Projektabhängigkeiten installieren: `npm run setup`.
2. Tauri-Entwicklungsstart: `npm run tauri -- dev`.
3. Formatieren: `npm run format`.
4. Statische Prüfungen: `npm run check`.
5. Tests: `npm test`; Regel-Fixtures: `npm run test:guardrails`.
6. Coverage: `npm run coverage`. Vor vorhandener Produktlogik lautet die Abnahme „nicht anwendbar“; die positiven/negativen instrumentierten Regel-Fixtures bleiben verbindlich.
7. Vollständige Prüfung: `npm run verify`. Zusätzlich benötigt: cargo-deny 0.19.0 und gitleaks 8.30.1 (vorhandene lokale Werkzeuge). Der Coverage-Lauf verwendet das lokale LLVM 21 aus Xcode passend zu rustc 1.93.0. Setup/Verify ändern keine globale Toolchain.
8. Lokales Release bauen: `npm run release`. Ein erzeugtes Bundle ist erst nach den Installations-/Laufzeitprüfungen im Plan ein abgenommenes Artefakt.

Installation, Berechtigungen, Modellsetup, Bedienung und nachgewiesene Grenzen werden mit den tatsächlichen Implementierungs- und Abnahmeschritten ergänzt. Audiodaten und Transkripte werden weder eingecheckt noch in Diagnoselogs geschrieben.
