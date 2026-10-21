# Abgleich mit Florians AI-Guardrails

Quelle: [florianbuetow/ai-guardrails](https://github.com/florianbuetow/ai-guardrails/tree/e15f537328a320092e53a674ac1dbff464c3a259),
Revision `e15f537328a320092e53a674ac1dbff464c3a259`, gelesen am 21.10.2026.
Geprüft wurden README, AGENTS-Vorlagen, Justfiles, Rust-/React-Checks und die
Semgrep-Regeln der Ordner `blueprints/rust-cli-base` und
`blueprints/react-vite-typescript-base`. Lokale Rohkopien und Dateiliste liegen in
`artifacts/final-guardrails-2026-10-21/upstream/`.

Die Vorlagen sind ein Vergleichsmaßstab, keine nachträglich angewandte
Projektschablone. **Erfüllt** benennt konkrete Durchsetzung, **ersetzt** eine
andere Umsetzung desselben Zwecks, **abweichend** eine bewusste Entscheidung.
Keine Vollübernahme und keine lückenlose Sicherheitsprüfung wird behauptet.

## Rust

| Vorlagenregel/Zweck | Stand hier | Umsetzung / Beleg / Entscheidung |
|---|---|---|
| Rust 2024 und reproduzierbarer Einstieg | Erfüllt | Edition 2024, gepinnte Toolchain, Cargo.lock, `npm run setup` |
| rustfmt / cargo check | Erfüllt | `npm run check`: Format, Clippy aller Targets und dadurch Typprüfung |
| Clippy pedantic/nursery/cargo vollständig | Abweichend | Defaultgruppen plus gezielte deny-Regeln für Panic, ignorierte Resultate und unbegründete Ausnahmen; kein globales Styling-Warnungsrauschen als Korrektheitsbeleg |
| unwrap / leeres expect verhindern | Erfüllt, strenger | Eigene Rust-Pfade verbieten unwrap/expect/panic insgesamt; Clippy plus Quellguardrail |
| Fehler nicht durch `let _ = Result` verschlucken | Erfüllt | unused_must_use und let_underscore_must_use auf deny; Verhaltenstests für die relevanten Fehlerpfade |
| Alle unwrap_or-/Default-Fälle verbieten | Abweichend | Fehlerfortpflanzung und dokumentiertes Fail-closed-Verhalten sind maßgeblich. `Outcome::Cancelled` als sicherer Zustand ist zulässig; mechanisches Umschreiben in match verbessert das nicht |
| allow verbieten; expect nur begründet | Abweichend | Begründete allow/expect sind zulässig, ungeklärte Ausnahmen scheitern. Compile-Time-Plattformgrenzen und native Verträge brauchen gezielte Ausnahmen |
| #[ignore] verbieten | Abweichend | Begründete, echte OS-/manuelle Tests bleiben explizit ignoriert und werden nicht als bestanden gezählt. Portable Verträge müssen ohne menschliche Eingabe laufen |
| Eigene unsafe-Blöcke verbieten | Ersetzt | Kern/Host verbieten unsafe; erforderliche native FFI ist in einem getrennten Crate, mit Sicherheitsbegründung. Ein Verbot auch dort widerspräche der systemnahen Aufgabe |
| cargo-geiger über Abhängigkeiten | Nicht übernommen | Ein Unsafe-Zähler über MLX/Whisper/AppKit beweist keine FFI-Sicherheit. Herkunft, Pins, native Grenzen, Lizenzen und konkrete Vertragsreviews sind die verwendeten Kontrollen |
| cargo-machete | Erfüllt | Gepinnt 0.9.2, `check:dependencies:rust`, eigene Workspace-Crates; keine Upstream-Manifeste künstlich bereinigen |
| cargo-deny | Erfüllt | Vorhandenes `deny.toml`, `cargo deny check` im vollständigen Verify-Kommando |
| Semgrep-Regeln | Zweck ersetzt/Abweichungen oben | Rust-Compiler-/Clippyregeln und getestete Quellchecks; kein zweiter Scanner nur für identische Muster |
| cargo-nextest / grcov | Zweck ersetzt | Cargo-Tests plus begrenzte autonome Profile, LLVM-/Frontend-Coverage mit benannten Schwellen; kein zusätzlicher Runner für dieselben Tests |
| codespell | Nicht übernommen | Gemischt deutsch/englische Dokumentation und Fach-/Modellnamen; redaktioneller Abschluss statt großer Ignore-Wortliste als scheinbare Qualitätsgarantie |
| Alle public items dokumentieren | Teilweise, bewusst fokussiert | Öffentliche System-/FFI-Verträge und Architekturpfade dokumentieren; Rust-Hostmodule sind kein veröffentlichtes Bibliotheks-SDK. Keine erfundenen trivialisierenden Docstrings als Vollständigkeitskennzahl |
| Nur Cargo statt isoliertem rustc | Abweichend | Cargo für App/Integration; sehr kleine Harnesses ohne Modell-/Tauri-Link dienen schnellen Negativbelegen. Kein solcher Harness ersetzt den App-Build |

## Workflow und Agentenregeln

Die tatsächlichen Regeln, Kommandos und Ausnahmen stehen in den jeweiligen
Konfigurationen und [guardrails.md](guardrails.md). Die historische technische
Freigabe wird nicht als Beweis für später neu eingeführte Regeln ausgegeben.
