# Begrenzter Quellentransfer: nativer Einstellungsfokus

Der Fix `b799936c0a399d07a0785ba7c736e3c3ca2c82a1` ändert im normalen
Produkt ausschließlich `NativePreferences::show`. Der
[maschinenlesbare Vergleich](native-preferences-focus-transfer-2026-10-19.json)
bindet den vollständigen geprüften Quellenbereich, vorhandene RED/GREEN-Logs
und die neue normale Binärdatei. Dieser Audit führte keine Tests, Builds,
UI-Aktionen, Aufnahmen oder Inferenz aus.

## R11 bleibt der ursprüngliche tatsächliche Lauf

Der [globale Fünf-Minuten-Lauf](single-press-global-2026-10-19.md) bleibt an
R11-Binary `3310f2a3ec37d44a767a10e93958f491e7aeb92aacc728a0c428a20955579081`
gebunden: einmaliger Empfang von 3300 UTF-8-Bytes, bytegleich zum Transkript;
getrennte WER 28/425 = 6,59 %, mit dokumentierten Erkennungsfehlern. Er wird
weder wiederholt noch als tatsächlich ausgeführter Lauf des neuen Binarys
umetikettiert. R11-Archiv und historische JSON-Werte bleiben unverändert.

Der alte Paketquellstand ist `9e1ba8a`; die wirkliche Buildvorbereitung nennt
`68b07a9` plus eingefrorene Arbeitsdateien. Alle fünf dortigen Vorbereitungspins
stimmen mit dem Paketstand und dem neuen Ziel überein. Es wird kein sauberer
historischer Commitbuild erfunden.

## Tatsächlich geprüfte Gleichheit und Differenz

- Von den 19 R11-Paketquellenpins sind 18 bytegleich; nur
  `crates/macos-integration/src/native_preferences.rs` ändert sich.
- Alle **13 Quellenpins des finalen Reference-Manifests** sind am R11-Stand,
  an `b799936` und in den aktuellen Dateien hashgleich: Modell-/VAD-Definition,
  Policy, Engine/Vorbereitung, Audio/DSP, Runtime/Service, gepinnte native
  Whisperquellen und Lockfile. Das Manifest bleibt unverändert.
- Die gesamten Git-Bäume `src-tauri`, `crates/dictation-core`, `vendor`,
  `.cargo`, `src` und `config` sind identisch. Der vollständige native
  `src`-Baum unterscheidet sich nur in der genannten Datei. Root-/Native-
  Manifeste, npm-/Cargo-Lockfiles und Build-Einstiege sind unverändert.
  Damit sind auch Hotkey, Target und Delivery unverändert gebunden.
- Weitere Codeänderungen betreffen drei Entwicklerbeispiele und zwei native
  Testdateien. Sie werden als solche ausgewiesen, nicht als Änderungen des
  normalen Runtime-Einstiegs. Dokumentations-/Tooländerungen sind keine
  implizite neue Runtimeidentität.

Die einzige normale Produktdifferenz sind **elf zusätzliche Zeilen** im
`show`-Bereich 753–773: Nach der bisherigen Appaktivierung und Fensteranzeige
wird der bereits konfigurierte `initialFirstResponder` gesetzt, wenn noch
kein Responder beziehungsweise nur das eigene Fenster Responder ist.
Bestehender Control-/Textfokus bleibt erhalten. Weder Systempräferenz noch
Mikrofon-, Decode-, Ziel- oder Versandregel ändern sich. Der unveränderte
Main-Aufrufer verwendet `show` beim ausdrücklich angeforderten Öffnen der
Einstellungen.

## Bereits vorhandenes ausführbares Feedback

Der echte öffentliche Lifecycle-Vertrag scheiterte vor dem Fix an
„show/reopen did not establish the configured initial tab responder“.
Danach bestanden initiales Öffnen, drei Close/Reopen-Folgen, sichtbarer eigener
Hauptfensterbestand und Erhaltung eines separat vorbereiteten Textresponders.
Auch der normale Preferences-Vertrag und striktes Clippy bestanden. Logs und
Quellhashes liegen gebunden unter
`artifacts/native-preferences-show-focus-fix-v1/`.

Die unabhängige gezielte Quellprüfung ist laut Root geschlossen. Die eigene
Probe v3 zeigte laut Root Pfeil-/Leertasten-Tabwechsel und blauen Fokusring;
das ist ein eigener Probeumfang und noch kein neuer installierter Produktlauf.
Keine dieser Prüfungen wurde für diesen Quellenvergleich erneut ausgeführt.

## Neues Artefakt und noch getrennte Sichtprüfung

Root hat den normalen Build aus `b799936` beendet und signaturgeprüft.
Die konservierte Binärdatei ist tatsächlich gehasht:
`709da0a462109526edbffbaa190f41718add6b8a657dd4b17a5a7f957d282e63`.
`artifacts/native-preferences-final-normal-v1/preparation.json` und Buildlog
sind gebunden. Fingerprint: ausschließlich `default`; kein Measurement,
kein Test-Control und kein `LSEnvironment`. Eine neue physische G-Ausführung
wird nicht behauptet.

Der [tatsächliche installierte UI-Readback](native-preferences-keyboard-2026-10-19.md)
ist nun zusätzlich gebunden: PID 30267, sichtbarer blauer Tab-Fokusring,
Pfeil-/Leertastenbedienung, Wiederöffnung und beide erhaltenen Rechte.
Anschließend bereit und leer; Systemstandard/Deutsch/⌥D unverändert. Dieser
Root-Befund ist keine neue Aufnahme oder G-Wiederholung. FullKeyboardAccess
blieb aus; eine vollständige Tab-Runde über sämtliche Controls wird nicht
behauptet. Der unabhängige Delta-Review hat inzwischen Quellen, Hashes, Screenshots und
JSONL geprüft und den sichtbaren nativen Tab-Fokus ohne blockierenden Befund
bestätigt.
