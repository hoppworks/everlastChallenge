# Unabhängiger Quellen- und Nachweisreview · 21.10.2026

Prüfung durch einen unabhängigen Codex-Subagenten in derselben lokalen Umgebung.
Produktcode, Tests, Buildkonfiguration und Laufzeitzustand wurden dabei nicht
geändert; dieser Bericht war das einzige geschriebene Artefakt.

## Umfang und Ergebnis

Geprüft wurde das zusammenhängende Änderungspaket auf Basis `e7cabfb` mit
nativer Korrektur `4ac9c55`, experimenteller Controller-/Decodertrennung
`ac78ecc`, Anzeigenamen `d1602b5` und den damaligen Guardrail-/Abdeckungs-/
Dokumentationsänderungen bei `6599669342ad7892f6a0d3ebb1516bf52760a235`.
Die Hashliste bindet die gelesenen wesentlichen Implementierungsdateien.
Das ursprüngliche externe Reviewziel `2d5d0a1` ist ein älterer Main-Stand.

**Kein verbleibender blockierender Quellenbefund im geprüften Paket.**
Diese Quellen-/Nachweisprüfung bestätigt weder den damals ausstehenden
Gesamttest noch einen frischen Release, Merge, eine Installation oder physische
Produktabnahme.

## Projektregeln

- Rust-Besitz bleibt ausdrücklich geregelt: Clipboardzustand auf dem
  App-Hauptthread, Warten auf dem nativen Worker. Keine neuen unsafe-Blöcke
  oder losgelösten Aufräum-Worker im Anwendungscode.
- FullPrefix-Controller/-Decoder sind aus normalen Builds ausgeschlossen.
  Verbliebene Audiohelfer und historische Messprogramme wurden nicht entfernt.
- Typinformierte ESLint-Regeln lehnen verlorene/falsch verwendete Promises
  und explizites `any` ab. Striktes TypeScript, Oxlint und Vertragsgenerierung
  bleiben erhalten. Die Quellenprüfung erfasst auch Anwendungsskripte;
  negative Regelfixtures sind bewusst ausgeschlossen.
- Der Architekturgraph verbietet Rückabhängigkeiten zum Frontend und Zyklen.
  Kernregeln erfassen Buildabhängigkeiten. Die Produkt-Featureprüfung folgt
  lokalen Aliasen, Zyklen, Workspace-Anforderungen und Crate-Übergängen;
  vorherige Umgehungen durch direkte Features/Aliase wurden korrigiert.
- Native Quellen werden gegen das gepinnte Upstream-Inventar geprüft. Dieselbe
  Eingangsliste steuert Quellenprüfung und Buildkopie. Bei den vier geänderten
  nativen Eingaben wurde kein weiterer Unterschied gefunden.
- CI trennt portable Prüfungen vom ausdrücklich konfigurierten Mac-Runner.
  Actions sind an geprüfte Commits gebunden, Zugangsdaten werden nicht
  gespeichert, Rechte bleiben lesend und Secret-Scans sind enthalten.
  Lokale Gesamtprüfung und optionale native CI erfassen Host-Abdeckung.
  YAML und lokale Belege beweisen weder einen Remote-Lauf noch Branchschutz.

## Anforderungen und Korrektheit

Rust/Tauri 2, sichtbare React-/TypeScript-Oberfläche und native Aufnahme,
Tastenkürzel und Übergabe bleiben erhalten. Der Anzeigename lautet
Everlast Coding Challenge; die Bundle-ID bleibt
`de.hoppworks.everlast-dictation`.

Der Timeoutfehler wird an der Besitzgrenze korrigiert: `SentPaste` startet die
500-ms-Frist nach dem tatsächlichen Ereignisversand. Frühes Aufräumen gibt eine
aufgeschobene Frist zurück und übernimmt den nativen Eintrag nicht.
`NativeBackend::cleanup` wartet außerhalb des Hauptthreads und versucht es
später erneut; der Shutdownbesitz bleibt erhalten. Verlassene erfolgreiche
Versandaufträge räumen nicht vorzeitig auf. Normale Antworten werden weiter
zugestellt: Eine erste fehlerhafte Auslassung wurde im Review erkannt und durch
einen gezielten Test normaler/aufgeschobener Antworten korrigiert.

Tests ordnen tatsächlichen Abschluss/Abbruch und Aufräumen kontrolliert um den
Timeout und seine exakte Frist. Sie belegen keinen realen Tastatur-/Clipboard-
OS-Lauf. Eine über das Zeitlimit blockierte GUI kann weiterhin sichtbar
fehlschlagen. Externer Empfang und harte Inferenzunterbrechung werden nicht versprochen.

## Gelesene Nachweise

- `paste-green.log`: zwölf native Diensttests bestanden, einschließlich
  Timeout nach Versand und normaler/aufgeschobener Bestätigung.
- `read-window-tests.log`: beide nativen Hauptübergabetests bestanden.
  Frühes Aufräumen verbraucht den Besitz nicht; an der Frist ist es zulässig.
- `frontend-coverage.log`: 98.18 % Anweisungen, 95.58 % Zweige, 98.91 % Funktionen,
  98.97 % Zeilen; ausschließlich Frontendabdeckung.
- `coverage/host/cargo-output.jsonl`: 345 Tests bestanden, drei ignoriert,
  kein Fehler, 35.97 Sekunden. `host-coverage-final.log` bestätigt die folgenden
  Werte und eingehaltenen Untergrenzen.

| Hostdatei | Zeilenabdeckung | Erfasst/gesamt |
|---|---:|---:|
| `native_service.rs` | 70.1308 % | 965/1376 |
| `controller.rs` | 95.2199 % | 498/523 |
| `runtime.rs` | 89.2086 % | 1984/2224 |
| `setup.rs` | 78.4491 % | 870/1109 |
| `desktop.rs` | 59.9905 % | 1264/2107 |

Die damaligen dateibezogenen Untergrenzen waren 70/95/89/78/59 Prozent.
Instrumentierung enthält Inline-Tests und verwendet measurement/test-control.
Das ist weder reine Produktcode-Zweigabdeckung noch ein Sicherheitsbeweis oder
Abdeckung sämtlicher Abhängigkeiten. Das Entfernen von erzwungenem
link-dead-code korrigierte einen Linkerfehler ungenutzter Swift-FFI in
instrumentierten Tauri-Buildhelfern; der vorherige Fehlschlag bleibt erhalten.
Portable und Werkzeugnachweise wurden begrenzt übernommen. Der Gesamtlauf war
zum Reviewzeitpunkt noch nicht beendet. Builds, Produkt-, Mikrofon- und
Tastenkürzeltests wurden durch diesen Review nicht erneut ausgeführt.

## Ergänzender reiner MLX-Versuch

Unabhängig geprüft wurden eingefrorener Probehash, beide Korpushashes, alle
sechs WAV-Hashes, Ergebnisse, Zeit-/Speichernachweise und die exakte
Neuberechnung beider Qualitätsdateien mit dem gepinnten Auswerter.
Vier deutsche Ausgaben sind identisch; bei der fünften sinken die Wortfehler
von elf auf zehn bei 119 Referenzwörtern. Englisch steigt von null auf zwei
Fehler bei 34 Wörtern; „decertification“ wird zu „the certification“.

Einmalige Prozesszeiten: Hybrid 4.053284459 s, MLX 2.955995417 s.
Maximaler RSS: 1,343,356,928 beziehungsweise 1,736,900,608 Byte. Profile und
Checkpoints unterscheiden sich, Reihenfolge ist fest und OS-Caches können
abweichen. Reines ASR-MLX hat keinen CPP-ASR-Fallback, teilt aber native Silero-VAD.
Der Hybrid bleibt Produktpfad; universelle Geschwindigkeits-, Speicher-,
Qualitäts- oder neue finale Buildlatenz-Aussagen folgen daraus nicht.

## Grenzen

Diese Quellenprüfung belegt keine späteren Gesamttests oder Paketprüfungen.
GUI- und physische Tastennachweise gelten nur für ihre jeweils benannten
Binärdateien.

## Hashes der geprüften Dateien

| Datei | SHA-256 |
|---|---|
| `crates/macos-integration/src/main_delivery.rs` | `4dc8b402d10f284a5546f5e0a38755b560cf90a4a7594b9fcf6d685d68a34b5d` |
| `crates/macos-integration/src/paste.rs` | `4ca640e59abd6c3c31a40d7ceb5e96fb2375cea96dd87608c5436f8a53d1c714` |
| `src-tauri/src/native_service.rs` | `f734b33eef253f833b627a58a3ff98e642a0e89e939fd0f1b1876d3367c29e1a` |
| `src-tauri/src/controller.rs` | `4008a6cd58511818bd9df4013d460a2bf70dbcc9969f91c00d5044f4526390d3` |
| `src-tauri/src/inference_service.rs` | `e892c43d2a179ec04bdaa00da8af649aedff571d72a8fe4463839162fa599901` |
| `src-tauri/src/pipeline.rs` | `a4b73193b0db5f68b7f135ed0b8c48e03c7cb1ed854df35aa3dda1ad2598fd64` |
| `src-tauri/src/runtime.rs` | `df306a4473e8500f04462a36f723492a2d83b5409ce751fa405a6e1f507fcfef` |
| `src-tauri/src/transcription.rs` | `923c44f155aa0adca7636a9f45970067c7656d71682d38fe3288f0ca0578696c` |
| `scripts/architecture.mjs` | `39cb9ebfbdd97ed6d6edf430265399f9b8bfe61497941210c51b90df5ae7d03c` |
| `scripts/architecture.test.mjs` | `9ffa15e7d1345b264c6b601e355554313bfbbb02e7e4826d324d79ca3586e43f` |
| `scripts/coverage.mjs` | `3a697c2161186bbd4d21cb30fbdb5f36c673af039d45f39a008cca5a04c2ffc0` |
| `scripts/coverage-host.mjs` | `82de53e76f16b670a37cc62372395cf0866f2a23dd3c49393b91489f619ffce8` |
| `scripts/guardrails.mjs` | `923f193a039771b91995972f00cffe3b36f71b4125f799f564897aa571042357` |
| `eslint.config.mjs` | `13fd1496c97107662d903f6a9c49063fc554a9773438790385274547b39d129d` |
| `config/coverage.json` | `e8cb58b2c175031ef8dd9110744c524614b6729e3e3296abc80d8f86287f125b` |
| `vendor/whisper-rs-sys/build.rs` | `67b02d2c3603a69aef992ebc8acffd4ebccc574ed4cfeda675a0e71a70c62761` |
| `vendor/whisper-rs-sys/native-inputs.txt` | `d38dd6f52f48346400b1a2e07ef0520f934f1c077e5d760fee06be152021b122` |
| `.github/workflows/quality.yml` | `3d47ec8266ab4a049deb25fec7fe4de87a1c9adc136610979caa5b0ec3129825` |
| `package.json` | `08205010e51320ad04752ded9574b0b5f17c81a92fdd608de32c2c68fff39d86` |
