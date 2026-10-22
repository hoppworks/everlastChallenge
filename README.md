# Everlast Coding Challenge

Eine native Diktier-App für **Apple Silicon und macOS 27**, gebaut mit **Rust,
Tauri 2 und React/TypeScript**. Aufnahme, Stilleerkennung, lokale Transkription,
globale Tastenkürzel und Einfügen laufen im nativen Pfad. Die WebView zeigt und
steuert den Zustand; sie verwendet keine Browser-Mikrofonaufnahme.

Der Produktpfad ist ein **MLX-Hybrid**: optimierter MLX-Encoder, vollständiger
whisper.cpp-Decoder. Lange Eingaben und spätere Whisper-Fenster behalten den
qualifizierten CPP-Pfad. Reines MLX wurde untersucht, nach Qualitätsabweichungen
aber nicht als Produktstandard übernommen.

[Repository](https://github.com/hoppworks/everlastChallenge) ·
[CI-Läufe](https://github.com/hoppworks/everlastChallenge/actions)

## Download für macOS

**[App herunterladen — macOS, Apple Silicon (.zip)](https://github.com/hoppworks/everlastChallenge/releases/latest/download/Everlast-Coding-Challenge-macOS-arm64.zip)**

ZIP entpacken und die App nach Programme ziehen. Erfordert **macOS 27 oder neuer**
auf Apple Silicon. Die App ist lokal signiert, nicht Apple-notarisiert; macOS kann
beim ersten Öffnen einen Sicherheitshinweis anzeigen. Prüfsumme und Details stehen
im [aktuellen Release](https://github.com/hoppworks/everlastChallenge/releases/latest).

## In wenigen Minuten durch das Repository

1. [Technische Entscheidungen](docs/engineering-decisions.md): zentrale Invarianten und Abwägungen.
2. [Code-Wegweiser](docs/code-map.md): vom Aufnahmestart bis zum Einfügen.
3. [Guardrails](docs/guardrails.md): automatisierte Architektur- und Qualitätsregeln.
4. [Leistung](docs/performance.md) und [Vergleich](docs/comparison.md): Messwerte und Grenzen.
5. [KI-Nutzung](docs/ai-review.md): Werkzeuge und überprüfte Beispiele.
6. [Entwicklerwerkzeuge](docs/tooling.md): Auswertung, Korpus und reproduzierbare Prüfungen.

Der [Dokumentationsindex](docs/README.md) trennt aktuelle Einstiege von datierten
Versuchen und früheren Implementierungen.

## Fertige App starten

Den oben verlinkten ZIP entpacken und die `.app` nach Programme kopieren.
Die Modellgewichte sind nicht im Download enthalten. Für den einmaligen
Modelldownload wird Internet benötigt; anschließend erfolgt die Erkennung offline.

## Benutzen

1. App öffnen und die Modelle über die Einrichtung laden. Dateien werden auf
   Größe und Integrität geprüft; danach ist die Erkennung lokal und offline.
2. Mikrofonzugriff sowie für automatisches Einfügen die macOS-Bedienungshilfen
   freigeben.
3. Das Textfeld in der Zielanwendung fokussieren, **⌥D** drücken und sprechen.
4. **Nochmals ⌥D:** Aufnahme beenden, transkribieren und Text einfügen.
   **Escape:** Aufnahme verwerfen; dabei wird nichts eingefügt.

Der Aufnahmebutton der App erzeugt ein lokales Diktat. Für automatisches
Einfügen startet das globale Tastenkürzel die Aufnahme: So wird das Ziel vor
einem möglichen Fokuswechsel erfasst. Bei unsicherem oder verändertem Ziel
bleibt der Text in der App. Nach fünf Minuten endet die Aufnahme automatisch.

Kopieren übernimmt exakt den sichtbaren Text. Endet ein Hintergrunddienst,
bleibt der letzte Text kopierbar; die App erklärt den erforderlichen Neustart.
Ein gescheiterter Modelldownload kann dagegen ohne App-Neustart wiederholt werden.
Für Mikrofone bleibt ein unterstütztes Standardformat erhalten. Andernfalls wird
ein angebotenes Format mit 1–8 Kanälen und 8/16/24/32/44,1/48 kHz gewählt:
weniger Kanäle zuerst, dann höhere Rate, dann Float-/Signed-Formate. Gibt es kein
passendes Format, hilft die Fehlermeldung bei der Einstellung im Audio-MIDI-Setup.
CPAL kann dabei die nominale Rate des Audiogeräts systemweit umstellen; andere
Audioanwendungen können betroffen sein. Dies ist keine Garantie für jedes externe Interface.

Die App verwendet ausschließlich den MLX-Hybrid. **Einstellungen → Modell**
zeigt dessen Status und Herkunft. Ältere gespeicherte Modellwahlen werden beim
Start auf den Hybrid umgestellt; Mikrofon und Hotkey bleiben erhalten.
Deutsch ist die freigegebene Produktsprache. Weitere Sprachen und schwierige
Aufnahmen haben dokumentierte Qualitätsgrenzen.

Der eigene Anwendungscode und die aktiven Entwicklerwerkzeuge sind Rust und
TypeScript. Die eingebundenen Inferenzbibliotheken enthalten weiterhin C/C++
und Metal; deren unveränderte Upstream-Quellen können weitere Hilfssprachen
enthalten. Für den Appbetrieb wird kein Python benötigt.

## Architektur

| Grenze | Verantwortung |
|---|---|
| `src/` | React-Ansichten und validierte TypeScript-IPC-Verträge |
| `src-tauri/src/desktop.rs`, `runtime.rs` | Aktionen, Sitzungszustand und Lebenszyklus der Dienste |
| `crates/dictation-core/` | Sitzungskern und Audioaufbereitung ohne OS-/Inferenzabhängigkeit |
| `src-tauri/src/audio.rs` | CPAL-Aufnahme, begrenzter Ring, DSP und Stop-/Flush-Barriere |
| `src-tauri/src/inference_service.rs`, `transcription/` | Serieller Modellbesitz und lokale Erkennung |
| `crates/macos-integration/` | Systemrechte, Zielprüfung, Clipboard und native Übergabe |

Ein Abbruch entzieht dem Ergebnis seine Veröffentlichungsberechtigung; er
verspricht keine harte Unterbrechung eines laufenden GPU-Kernels. Ein versandtes
Paste-Ereignis ist keine Empfangsbestätigung. Diese Unterschiede sind in den
Zustandsmodellen und Verhaltenstests ausdrücklich abgebildet.

## Entwickeln und bauen

Qualifiziert ist **Apple Silicon mit macOS 27**. Auf dem Prüfhost: macOS 27.0.1,
Xcode 27.1 (27A9275) mit separat installierter Metal Toolchain, Node 22.23.1 /
npm 10.9.8, Rust 1.99.1 und CMake 4.4.4. Andere Hostkombinationen sind nicht als
getestet ausgewiesen. Unterstützt sind Node 22 ab 22.22.2, Node 24 ab 24.15 sowie Node ab 26.
Die Mindestversion für CMake ist 3.25.

Vor dem ersten Setup auf einem neuen Mac:

1. Volles Xcode 27 installieren, einmal öffnen und die Erstinstallation/Lizenz
   abschließen. Falls mehrere Installationen bestehen, Xcode in dessen
   Einstellungen unter Locations → Command Line Tools auswählen. Command Line
   Tools allein ersetzen das benötigte Xcode/Metal-SDK nicht.
2. Metal Toolchain über Xcode installieren (`xcodebuild -downloadComponent
   MetalToolchain`); `xcrun --find metal` muss erfolgreich sein.
3. Node.js (inklusive npm), Git, **rustup** und **CMake** installieren. CMake muss
   im `PATH` stehen; bei vorhandenem Homebrew etwa `brew install cmake`.
   Die Rust-Installation erfolgt über den rustup-Installer. `rust-toolchain.toml`
   pinnt die Toolchain, installiert aber rustup selbst nicht.
4. Im frischen Checkout `rustup show` ausführen. Dadurch wird Rust 1.99.1 samt
   rustfmt, Clippy, LLVM-Werkzeugen und Apple-Silicon-Target bereitgestellt.

```sh
npm run doctor
npm run setup
npm run tauri -- dev
```

`doctor` prüft Voraussetzungen ohne Installation, Download oder Änderung der
Systemeinstellungen. Fehlende Werkzeuge werden einzeln mit Abhilfe gemeldet.
Setup benötigt Internet für die gepinnten npm-/Cargo-Abhängigkeiten und
cargo-machete 0.9.2; es installiert außerdem den versionierten lokalen Commit-Hook.
Der Modell-Download in der App ist davon getrennt. `npm run dev` startet nur die
Oberfläche; für native Funktionen die Tauri-App verwenden.

```sh
npm run release
```

Der Releasehelper bindet die tatsächlich gebaute `mlx.metallib` ein und prüft
Bundle und Mikrofondeklaration. Das Ergebnis liegt standardmäßig unter
`target/aarch64-apple-darwin/release/bundle/macos/Everlast Coding Challenge.app`.
Der Befehl installiert und veröffentlicht nichts. Die Standard-Ad-hoc-Signatur
ist keine Apple-Notarisierung. `npm run release:local` nutzt eine separat
konfigurierte lokale Entwicklungsidentität; Schlüssel und die private
`.everlast-signing.json` gehören nicht ins Repository. Beim Kopieren signierter
Apps müssen Metadaten erhalten bleiben, etwa mit `ditto`.

## Prüfen ohne manuelle Tastendrücke

```sh
npm run check:fast
npm run test:portable
npm run check
npm run test:autonomous
```

Der schnelle Check umfasst Typen, Lints, Formatierung, Modul-/Featuregrenzen und
JS/TS-Abhängigkeitshygiene. `check` ergänzt Rust-Clippy, generierte Verträge und
Rust-Abhängigkeitshygiene. Der [autonome Testleitfaden](docs/autonomous-testing.md)
beschreibt begrenzte Softwareprofile und trennt sie von echten OS-Abnahmen.
Der Entwickler-Steuerkanal bleibt im normalen Produktbuild deaktiviert.

Bei Änderungen werden vorhandene Belege anhand ihrer Quell-, Modell- und
Parameterbindung wiederverwendet. Nur betroffene Verträge werden erneut geprüft;
keine neue ASR-Kampagne für Dokumentation oder Formatierung. Native Timingläufe
werden nicht parallel zu Builds gestartet. Die portable GitHub-CI ist
konfiguriert; eine Workflowdatei allein belegt keinen erfolgreichen Remote-Lauf.

## Ergebnisse, Herkunft und Grenzen

Die [Leistungsübersicht](docs/performance.md) beschreibt die erhaltenen
Messstrecken und deren Grenzen. Historische Messwerte sind keine garantierte
Latenz der aktuellen sichtbaren App. Im [historischen Voicely-Helfervergleich](docs/comparison.md)
waren die Prozessmediane 36,4–39,1 % kürzer. Das umfasst Modellladen und
Prozessende, aber weder Aufnahme noch Einfügen; der aktuelle FP16-Release wurde
damit nicht neu gemessen.

Bibliotheken, Modelle und lokale Patches sind unter
[Abhängigkeiten](docs/dependencies.md) und [Lizenzen](docs/licenses/third-party/README.md)
zugeordnet.

### Kompakte Palette

Die Hauptansicht zeigt Status, Mikrofonpegel und Aufnahmeaktion. „Letzter Text“
und „Aufnahmedetails“ lassen sich aufklappen; neue Ergebnisse öffnen den Text
auch dann, wenn zwei Diktate gleich lauten. Die Einstellungen öffnen sich über
das Zahnradsymbol oder ⌘,. Das Fenster wächst für Text und Einstellungen.
Hell-/Dunkelmodus folgen der macOS-Einstellung. Die Balken zeigen den tatsächlichen
Mikrofonpegel, kein Frequenzspektrum. ⌘R startet/stoppt ein Diktat im App-Fenster;
das konfigurierte globale Tastenkürzel und Escape bleiben systemweit verfügbar.

## Lizenz

Der Anwendungscode steht entsprechend der Manifestdeklaration unter [MIT](LICENSE).
Drittanbieter behalten ihre eigenen [Lizenzen und Herkunftshinweise](docs/licenses/third-party/README.md).

Für **realfft 3.5.0** enthält der verwendete Upstream-Stand keine vollständige
originale MIT-Lizenzdatei mit Copyrightnotice. Die unveränderte MIT-Deklaration,
Autorenmetadaten und eine ausdrücklich gekennzeichnete standardisierte
MIT-Ergänzung liegen bei; ein Copyrightinhaber oder Jahr wird nicht erfunden.
Diese verbleibende Herkunftsgrenze ist im
[Lizenznachtrag](docs/verification/license-completion-2026-10-21.md) dokumentiert.
