# Everlast Dictation

Native Diktier-App für Apple Silicon und macOS 27. Rust/Tauri 2 mit TypeScript-Oberfläche, lokale Whisper-Inferenz und systemnahe Audioaufnahme/Textübergabe.

**Entwicklungsstand:** Der native Durchstich mit Modellsetup, Sitzungsbesitzer, Hotkeys und Tauri-Oberfläche ist implementiert. App-/DMG-Build und Modellsetup im nativen Fenster sind geprüft. Der installierte Mikrofon-/TextEdit-Ablauf und die weiteren Produktabnahmen stehen noch aus; dies ist noch kein abgenommenes Diktierprodukt. Fortschritt, tatsächliche Prüfungen und offene Abnahmen stehen im [Ausführungsprotokoll](docs/execution-log.md). Der [Implementierungsplan](IMPLEMENTATION_PLAN.md) enthält den vollständigen Zielumfang.

## Lokal entwickeln

Geprüfte Ausgangsumgebung: macOS 27.0.1, arm64, Rust/Cargo 1.93.0, Node 22.23.1, npm 10.9.8 und Xcode 27.1. Kein anderer Rechner wurde geprüft.

1. Projektabhängigkeiten installieren: `npm run setup`.
2. Tauri-Entwicklungsstart: `npm run tauri -- dev`.
3. Formatieren: `npm run format`.
4. Statische Prüfungen: `npm run check`.
5. Tests: `npm test`; Regel-Fixtures: `npm run test:guardrails`.
6. Coverage: `npm run coverage`. Die Mindestwerte sind 90 % Kern-Zeilen und jeweils 80 % Frontend-Zeilen, Statements, Funktionen und Zweige. Die positiven/negativen instrumentierten Regel-Fixtures prüfen, dass die Grenzen tatsächlich durchgesetzt werden.
7. Vollständige Prüfung: `npm run verify`. Zusätzlich benötigt: cargo-deny 0.19.0 und gitleaks 8.30.1 (vorhandene lokale Werkzeuge). Der Coverage-Lauf verwendet das lokale LLVM 21 aus Xcode passend zu rustc 1.93.0. Setup/Verify ändern keine globale Toolchain.
8. Lokales Release bauen: `npm run release`. Der Befehl prüft anschließend Signatur, Mikrofon-Nutzungsbeschreibung und signiertes Audio-Input-Entitlement. Ein vorhandenes Bundle lässt sich mit `npm run verify:bundle` erneut prüfen. Ein erzeugtes Bundle ist erst nach den Installations-/Laufzeitprüfungen im Plan ein abgenommenes Artefakt.

## Aktuellen Entwicklungsstand ausprobieren

Der lokale Release-Build liegt unter `target/aarch64-apple-darwin/release/bundle/`: als App im Unterordner `macos`, als DMG in `dmg`. Dieser direkte Entwicklungsstart ersetzt die noch offene Prüfung des heruntergeladenen, quarantänierten Installationsartefakts nicht.

Beim Start wird nur der eigene Modellcache geprüft. Mit **Modell laden** lässt sich das vorläufige Referenzmodell `large-v3-turbo-q5_0` (574 MB) von der im Fenster verlinkten, festgelegten Hugging-Face-Revision herunterladen. Die App prüft Größe und SHA-256 vor dem Laden. **Einrichtung abbrechen** beendet den Vorgang; ein neuer Versuch muss ausdrücklich gestartet werden. Ein erfolgreicher Download samt Modellinitialisierung ist im nativen Bundle geprüft. Die endgültige Modellwahl folgt erst nach den Qualitäts- und Leistungsmessungen. Im aktuellen Quellstand sperren Prüfung eines vorhandenen Cacheordners und Download den betreffenden Modellbestand gegen parallele Einrichtung. Beim Neustart werden ausschließlich eindeutig zugehörige Teildateien nachweislich beendeter Prozesse entfernt; aktive oder unklare Dateien bleiben erhalten. Eine belegte Sperre wird mit bewusstem erneutem Versuch gemeldet.

**Einstellungen** bietet im aktuellen Quellstand Mikrofonwahl, ein physisches Tastenkürzel mit mindestens Option, Command oder Control sowie den Modellstatus. Änderungen werden ausdrücklich gespeichert und sind während eines Diktats gesperrt. Nicht eindeutige Mikrofonnamen bleiben gesperrt; bei Registrierungs- oder Speicherfehlern wird der vorherige Zustand wiederhergestellt. Deutsch ist wählbar, Englisch folgt erst nach seiner Qualitätsabnahme. Außerdem enthält die Ansicht die ausdrücklichen Aktionen für Mikrofon und Bedienungshilfen. macOS verwaltet diese Freigaben. Mikrofonzugriff ist für Aufnahmen nötig; Bedienungshilfen werden für die automatische Übergabe an eine andere App benötigt. Die App bestätigt keine Freigabe stellvertretend. Nach Änderungen in den Systemeinstellungen die App wieder in den Vordergrund holen, damit sie den aktuellen Status liest.

Der implementierte Bedienweg ist **Aufnehmen → Stoppen** im Fenster oder die physische Tastenkombination **⌥D** im Zielprogramm. **Escape** bricht eine aktive Sitzung ab. Ein im Fenster gestartetes Diktat bleibt lokal; ein Hotkey-Diktat versucht die Übergabe ausschließlich an das zuvor geprüfte Ziel. Bei unsicherem Ziel bleibt der Text in der App zum bewussten Kopieren erhalten. „Einfügebefehl gesendet“ bestätigt nur den Versand des Tastaturereignisses, nicht den Inhalt im Zielprogramm. Diese Aufnahme- und Übergabewege benötigen noch ihre reale installierte Abnahme.
