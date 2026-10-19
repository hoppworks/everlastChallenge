# Everlast Dictation

Native Diktier-App für Apple Silicon und macOS 27. Rust/Tauri 2 mit TypeScript-Oberfläche, lokale Whisper-Inferenz und systemnahe Audioaufnahme/Textübergabe.

**Testversion (19.10.2026):** Die stabile Referenzfassung ist als lokale Testversion bereitgestellt. Der [Paketbeleg](docs/verification/testversion-2026-10-19.md) nennt Quellstand, Prüfsummen und tatsächliche Laufzeitprüfung. Die experimentellen FullPrefix-/Kontext-1400-Optimierungen sind nicht aktiv. Die Referenz hat ihre dokumentierte Modellqualitätsprüfung bestanden, erreicht aber das geplante Stop-bis-Text-Ziel von einer Sekunde nicht durchgehend. Die vollständige Leistungs-, Geräte-, Installations- und Produktabnahme bleibt offen. Installation und kurzer Funktionstest stehen in [TESTVERSION.md](TESTVERSION.md); tatsächliche Belege im [Ausführungsprotokoll](docs/execution-log.md), der vollständige Zielumfang im [Implementierungsplan](IMPLEMENTATION_PLAN.md).

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

Die lokalen P05-Werkzeuge für öffentlichen Prüfkorpus, Referenzparameter und
WER-Auswertung sind im [Korpus-Leitfaden](tests/fixtures/corpus/README.md)
dokumentiert. Dateibackend-Ergebnisse ersetzen keine Echtzeit- oder installierte
App-Abnahme.

Der lokale Release-Build liegt unter `target/aarch64-apple-darwin/release/bundle/`: als App im Unterordner `macos`, als DMG in `dmg`. Dieser direkte Entwicklungsstart ersetzt die noch offene Prüfung des heruntergeladenen, quarantänierten Installationsartefakts nicht.

Beim Start wird nur der eigene Modellcache geprüft. Mit **Modell laden** lässt sich das vorläufige Referenzmodell `large-v3-turbo-q5_0` (574 MB) von der im Fenster verlinkten, festgelegten Hugging-Face-Revision herunterladen. Die App prüft Größe und SHA-256 vor dem Laden. **Einrichtung abbrechen** beendet den Vorgang; ein neuer Versuch muss ausdrücklich gestartet werden. Ein erfolgreicher Download samt Modellinitialisierung ist im nativen Bundle geprüft. Die endgültige Modellwahl folgt erst nach den Qualitäts- und Leistungsmessungen. Zusätzlich benötigt der aktuelle Quellstand den fest gepinnten Silero-Sprachdetektor (0,9 MB). Die Einrichtung prüft beide Dateien und lädt fehlende Dateien nach ausdrücklichem Start; vorhandene geprüfte Dateien zählen zum Bereitstellungsfortschritt. Erst nach erfolgreichem Laden beider Modelle ist die Modelleinrichtung bereit. Der Sprachdetektor prüft die gesamte Aufnahme über eine getrennte pegelnormierte Kopie; bei erkannten Sprachanteilen erhält Whisper seine bisherigen Audiodaten unverändert. Fehler werden angezeigt und nicht als Stille behandelt. Im aktuellen Quellstand sperren Prüfung eines vorhandenen Cacheordners und Download den betreffenden Modellbestand gegen parallele Einrichtung. Beim Neustart werden ausschließlich eindeutig zugehörige Teildateien nachweislich beendeter Prozesse entfernt; aktive oder unklare Dateien bleiben erhalten. Eine belegte Sperre wird mit bewusstem erneutem Versuch gemeldet.

**Einstellungen** bietet im aktuellen Quellstand Mikrofonwahl, ein physisches Tastenkürzel mit mindestens Option, Command oder Control sowie den Modellstatus. Änderungen werden ausdrücklich gespeichert und sind während eines Diktats gesperrt. Nicht eindeutige Mikrofonnamen bleiben gesperrt; bei Registrierungs- oder Speicherfehlern wird der vorherige Zustand wiederhergestellt. Deutsch ist wählbar; Englisch ist in dieser Testversion noch nicht freigeschaltet. Außerdem enthält die Ansicht die ausdrücklichen Aktionen für Mikrofon und Bedienungshilfen. macOS verwaltet diese Freigaben. Mikrofonzugriff ist für Aufnahmen nötig; Bedienungshilfen werden für die automatische Übergabe an eine andere App benötigt. Die App bestätigt keine Freigabe stellvertretend. Nach Änderungen in den Systemeinstellungen die App wieder in den Vordergrund holen, damit sie den aktuellen Status liest.

Der implementierte Bedienweg ist **Aufnehmen → Stoppen** im Fenster oder die physische Tastenkombination **⌥D** im Zielprogramm. **Escape** bricht eine aktive Sitzung ab. Ein im Fenster gestartetes Diktat bleibt lokal; ein Hotkey-Diktat versucht die Übergabe ausschließlich an das zuvor geprüfte Ziel. Bei unsicherem Ziel bleibt der Text in der App zum bewussten Kopieren erhalten. „Einfügebefehl gesendet“ bestätigt nur den Versand des Tastaturereignisses, nicht den Inhalt im Zielprogramm. Diese Aufnahme- und Übergabewege benötigen noch ihre reale installierte Abnahme.

Die lokale Whisper-SDK-Korrektur und ihr numerischer Regressionsnachweis sind
in [Kalibrierung v3](docs/verification/calibration-v3.md) dokumentiert.
Bei Änderungen an vendertem C++ zuerst das betroffene sys-Buildprofil reinigen
und den aktiven OUT_DIR-Quellhash prüfen; Details in
[PATCHES.md](vendor/whisper-rs-sys/PATCHES.md). Die CPU-/Metal-Diagnose ist von
Qualitäts- und Echtzeitabnahme getrennt; Small-Metal zeigt noch eine dokumentierte
numerische Oracle-Abweichung.

## Fehler beheben

Die folgenden Schritte beschreiben den aktuellen Quellstand. Der ältere lokale
Testbuild kann davon abweichen; die finale installierte Abnahme steht noch aus.

| Beobachtung | Nächster Schritt |
| --- | --- |
| Aufnahme ist wegen fehlenden Mikrofonzugriffs gesperrt | Unter **Einstellungen** den Status prüfen. Bei **Noch nicht angefragt** die Aktion **Mikrofonzugriff anfragen** wählen und den macOS-Dialog selbst bestätigen. Bei einem anderen Status den dort angezeigten Hinweisen folgen. Anschließend zur App zurückkehren und den aktualisierten Status prüfen. |
| Automatisches Einfügen ist nicht verfügbar | Unter **Einstellungen** den Status der Bedienungshilfen prüfen und die Freigabe selbst erteilen. Nach einem anderen Build kann die frühere Freigabe nicht mehr zur laufenden App passen. Erst nach erneut angezeigter Freigabe einen neuen Versuch starten. |
| Das gewählte Mikrofon fehlt oder ist nicht eindeutig | Das Gerät verbinden, **Mikrofone aktualisieren** wählen und ein eindeutig bezeichnetes Gerät auswählen; anschließend ausdrücklich speichern. Die Abnahme mit externen Geräten und Bluetooth-Headsets ist noch offen. |
| Ein Modell fehlt, der Download scheitert oder die Einrichtung ist belegt | Die angezeigte Ursache prüfen und die Einrichtung bewusst erneut starten. Einen noch laufenden Einrichtungsvorgang zuerst beenden; Modell- oder Sperrdateien nicht währenddessen manuell entfernen. Für die erste Einrichtung wird Internet benötigt. Offline-Betrieb setzt beide vollständig geprüften und geladenen Modelle voraus. |
| Der erkannte Text bleibt in Everlast | Den Text prüfen, **Kopieren** wählen und bewusst im gewünschten Ziel einfügen. Ein unsicheres oder inzwischen gewechseltes Ziel wird zurückgehalten. Auch **Einfügebefehl gesendet** ist noch kein Beleg, dass die Ziel-App den Text übernommen hat. |
| Die App meldet nicht lesbare Einstellungen | **Einstellungen zurücksetzen** wählen und die Rückfrage bestätigen, wenn die bisherigen Einstellungen ersetzt werden dürfen. Mikrofon, Sprache, Tastenkürzel und Modellauswahl werden zurückgesetzt; heruntergeladene Modelle bleiben erhalten. |
