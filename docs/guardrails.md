# Ausführbare Qualitäts- und Architekturregeln

Diese Regeln wurden projektspezifisch umgesetzt, nicht als unveränderte Kopie
von ai-guardrails übernommen. Der Anspruch ist: ein versehentlicher Regelbruch
scheitert an einem reproduzierbaren Kommando, nicht erst am Gedächtnis des Autors.

| Regel | Durchsetzung |
|---|---|
| Kein unsafe im Kern oder Host | Rust `forbid(unsafe_code)` und Workspace-Lints; native FFI getrennt |
| Keine unbehandelten Rust-Fehler oder Panic-Abkürzungen | Clippy mit `-D warnings`, Regeln gegen unwrap/expect/panic und ignorierte Must-use-Werte |
| Keine still verlorenen Promises | Typinformierte ESLint-Regeln für Floating/Misused Promises; auch `void promise` braucht Fehlerbehandlung |
| Kein `any` oder unterdrückte TS-Fehler | ESLint no-explicit-any, Oxlint ban-ts-comment und strikte TypeScript-Optionen |
| UI greift nur über IPC auf Tauri zu | TypeScript-AST-Prüfung, einschließlich Reexports und dynamischer Imports |
| IPC hängt nicht von UI ab, keine Importzyklen | Auflösung lokaler Module und Graphprüfung in `scripts/architecture.mjs` |
| Kern bleibt ohne Betriebssystem-/Inferenzabhängigkeiten | Cargo-Metadaten gegen zugelassene Kernabhängigkeiten; native Integration darf nicht zum Host hochimportieren |
| Teststeuerung nicht in Produktionsdefaults | Feature-Graph wird auch durch Aliase und Zyklen verfolgt |
| Native Änderungen bauen inkrementell korrekt | Hashvergleich mit geprüftem crates.io-Upstream; dieselbe `native-inputs.txt` steuert die Buildkopie |
| Ausnahmen bleiben begründet | Source-Prüfung für Lintausnahmen und Coverage-Ignores; Tests enthalten negative Fälle |
| Keine still abweichenden IPC-Typen | Rust-Export und Vergleich der generierten TypeScript-Dateien |
| Reproduzierbare Abhängigkeiten | Lockfiles, direkte Versionspins, cargo-deny, npm audit, Gitleaks und vendorte Lizenz-/Herkunftsnachweise |

`npm run check:fast` prüft die schnellen Regeln ohne Audio, Modell-Download oder
Benutzerinteraktion. `npm run hooks:install` aktiviert den versionierten lokalen
Pre-Commit-Hook; `npm run setup` richtet ihn ebenfalls ein. Hooks können technisch
umgangen werden und ersetzen deshalb die CI nicht.

`npm run check` ergänzt Rust/Clippy und Vertragsabgleich. `npm run test:portable`
beweist zentrale Guardrails mit absichtlich ungültigen Eingaben und prüft die
Frontend-Verträge. `npm run test:autonomous` führt die Softwareverträge ohne
physische Tasten aus. `npm run coverage:host` misst zusätzlich die benannten Rust-Steuermodule. Coverage-Prozentwerte gelten nur für die benannten Dateien;
eine hohe Abdeckung ersetzt keine überprüfte Produktinvariante.

Die GitHub-Workflowdatei prüft portable Regeln und Secrets automatisch bei PRs und Änderungen
auf Main. Native Tauri-/Metal-Prüfungen verlangen einen ausdrücklich eingerichteten
passenden Mac-Runner. Branch Protection und ein erfolgreicher Remote-Lauf sind
Einstellungen beziehungsweise Ergebnisse auf GitHub, keine Eigenschaft dieser
lokalen YAML-Datei.

## Bewusst gewählte Invarianten

- Ein altes Sitzungsergebnis darf kein neues Diktat überschreiben.
- Abbruch entzieht die Berechtigung für spätere Veröffentlichung oder Übergabe.
- Ein Einfügeversuch wird nach irreversiblem Versand nicht automatisch wiederholt.
- Die Zwischenablage bleibt nach tatsächlichem Versand mindestens während der
  definierten Lesefrist erhalten, auch bei verspäteter Bestätigung.
- Puffer und Warteschlangen bleiben begrenzt; Audioüberlauf wird als Fehler behandelt.
- Modellbytes werden vor Verwendung vollständig geprüft.

Die entsprechenden Verhaltenstests liegen bei Sitzungskern, Controller,
Inferenz-/NativeService und Audio-Pipeline. Sie sind wichtiger als eine globale
Zahl von Tests oder künstliche Komplexitätslimits.

Die zusätzliche typinformierte Prüfung nutzt ausschließlich als Entwicklungstools
`eslint` 10.13.0 und `typescript-eslint` 8.72.0, mit aufgelösten Versionen im
`package-lock.json`. Sie ersetzt weder den TypeScript-Compiler noch die bestehenden
Oxlint-Regeln. TypeScript 7 prüft App und Werkzeuge; die AST-Prüfer verwenden
die separat gepinnte TypeScript-6-API. Die konkrete Auflösung steht im Lockfile.

## Zusätzliche Abschlussregeln

`npm run check:format` prüft jetzt Prettier mit gepinnter Version; generierte
Rust/TypeScript-Verträge bleiben beim Generatorformat und werden durch den
Vertragsvergleich geprüft. `npm run check:dependencies` verwendet gepinntes Knip
für ungenutzte Dateien, Exporte und npm-Abhängigkeiten. `gitleaks` ist dort als
extern installiertes Prüfwerkzeug ausgewiesen, nicht als npm-Paket versteckt.
`npm run check:dependencies:rust` verwendet cargo-machete 0.9.2 ausschließlich auf
den eigenen Crates einschließlich des Worker-Protokolls; vendorte
Upstream-Manifeste werden nicht umgeschrieben.
Setup und CI installieren diese Version explizit.

Setup und der portable CI-Job installieren außerdem `cargo-deny 0.20.2` für
denselben Advisory-, Lizenz-, Paket- und Quellencheck wie `npm run verify`.

Leere TypeScript-Catchblöcke, fokussierte/übersprungene Frontendtests, unkommentierte
ESLint-Ausnahmen und Format-/Typunterdrückungen werden abgelehnt. Hooks- und
ARIA-Regeln ergänzen die vorhandenen typinformierten Promiseprüfungen. Eine
bewusste Ausnahme ist `jsx-a11y/prefer-tag-over-role`: ein expliziter Statusbereich
bleibt `role="status"`, statt jede allgemeine Statusmeldung in ein HTML-`output`
umzuformen. Die ARIA-Rollen-/Attributprüfung bleibt aktiv.

Der genaue, revisionsgebundene Vergleich mit Florians Vorlagen und die bewusst
abweichenden Regeln stehen in [guardrail-crosswalk.md](guardrail-crosswalk.md).

Der Secret-Check erzeugt einen temporären Snapshot aller versionierten und
nicht ignorierten neuen Dateien. Er scannt damit auch neue Quelltexte vor dem
Commit, aber keine ignorierten lokalen Builds, Modelle oder privaten Sicherungen.
Versionierte Dateien bleiben selbst bei passender Ignore-Regel eingeschlossen.
Dies ist ein Repository-Quellscan, kein Scan sämtlicher lokaler Daten.

Hashgebundene Original-Lizenztexte unter `docs/licenses/third-party/texts/`
behalten ihre vollständigen Bytes einschließlich Whitespace. Nur dort ist
Gits Whitespaceprüfung per `.gitattributes` ausgenommen; eigene Quelltexte und
Dokumentation behalten die üblichen Prüfungen.

`mkfifo` ist wie `gitleaks` ein externes Werkzeug in Knip: Es gehört zum
POSIX-Testhost und erzeugt ausschließlich eine temporäre FIFO für die
Sonderdatei-Abweisung. Es ist keine npm-Abhängigkeit und kein Bestandteil der App.

Der isolierte `text-refinement-worker` gehört zu den Workspace-Format-, Clippy-
und Cargo-Prüfungen sowie zur expliziten Abhängigkeitsprüfung. Seine gezielten
Transport-/Snapshot-Tests laufen auch ohne MLX mit
`cargo test -p text-refinement-worker --no-default-features --locked`.
Diese Tests belegen weder Modellqualität noch den noch ausstehenden
Host-Supervisor oder nativen Textempfang.

`npm run check:refinement` und `npm run test:refinement` prüfen Protokoll,
Workermechanik und die Host-Prozessaufsicht ohne MLX auf portablen Runnern.
`npm run test:refinement:process` wählt nur die echten Prozess-Fixtures aus.
Das dafür benötigte Feature `test-fixture` ist ausschließlich explizit für Tests
aktiviert; die Architekturprüfung verbietet es in Produktionsdefaults und
normalen Abhängigkeitsfeatures. Der Fixture-Binärname wird nicht paketiert.
Die Fehlerpipe sammelt keine Texte und bricht nach 16 KiB Gesamtausgabe ab.

Die einzige Advisory-Ausnahme gilt für `RUSTSEC-2024-0436` (unmaintained
`paste`). Eine Paket-Sperre begrenzt dessen direkten Nutzer auf `tokenizers`
(im Lockfile 0.21.4).
Begründung und Ablösebedingung stehen in [dependencies.md](dependencies.md).
Neue Nutzer dieses Pakets, andere Advisory-IDs und eine nicht mehr benötigte
Ausnahme bleiben Fehler. Ein erfolgreicher Check mit dieser Ausnahme bedeutet
nicht, dass alle Abhängigkeiten aktiv gepflegt werden.

`npm run coverage:refinement` misst Protokoll, portable Workermechanik und die
Host-Prozessaufsicht dateiweise. Die Host-Coverage umfasst zusätzlich die
ausgelagerten Bereinigungs-, Einrichtungs- und Modellmodule. Fehlende Dateien,
leere Prüfbereiche und unterschrittene Einzelgrenzen scheitern unabhängig vom
Gesamtdurchschnitt. Der [Scope-Bericht](verification/refinement-coverage-2026-10-22.md)
nennt die Messwerte und die bewusst separat geprüften nativen Modellpfade.
