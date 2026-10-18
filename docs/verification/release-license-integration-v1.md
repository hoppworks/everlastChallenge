# Integration der vorbereiteten Lizenzsammlung

18.10.2026. Die separat geprüfte Sammlung auf Basis `b34914c` wurde aus
`release-license-material-b34914c-v1.tar.gz` importiert, Archiv-SHA256
`80f520d6b44147e68a095139779376231bbecca4ca1302a90179a1ce6e92da66`.
Das Archiv bleibt als privater Herkunftsbeleg erhalten.

Der lokale Readback prüfte alle 237 Archivmitglieder auf Pfadgrenzen und
regulären Datei-/Verzeichnistyp; Links wurden ausgeschlossen. Alle 228
Dokumentdateien stimmen mit Größe und SHA256 des geprüften Archivinventars
überein. Vier vorhandene Lizenztexte bleiben bytegleich. 223 neue Dokumente
wurden exklusiv angelegt; die Änderung von dependencies.md entspricht exakt
dem geprüften Vorschlag. Keine Laufzeitquellen, Abhängigkeiten oder Versionen
wurden dafür geändert.

Alle 315 Komponenteneinträge und 551 Textverweise wurden am tatsächlichen
lokalen Bestand gelesen; Größen und Hashes stimmen. Die Cargo-Komponenten
stimmen in Namen/Version und vorhandener Paketchecksumme mit Cargo.lock
überein. Die gebundenen Sourceinputs sind unverändert, mit einer ausdrücklich
geprüften Ausnahme: src-tauri/Cargo.toml enthält seither ausschließlich zwei
zusätzliche measurement-Exampleeinträge; seine Abhängigkeitsabschnitte sind
gegen die Basis unverändert. npm-Lockfile und Bundlekonfiguration sind gleich.
Die bestehende Resourcezuordnung erfasst docs/licenses/ unter licenses/.

Unveränderte Originaltexte behalten auch ursprüngliche Leerzeichen; sie werden
nicht zur Erfüllung einer Whitespaceprüfung redigiert. Dies gilt ebenso für
den ausdrücklich standardisierten MIT-Permission-/Disclaimer-Zusatz.

Die Sammlung umfasst vier produktive npm-Pakete und 311 konservative native
Normaldependency-Kandidaten. Das ist kein Nachweis der finalen Linkclosure.
Die fehlende originale realfft-Notice bleibt sichtbar; der gekennzeichnete
Standardzusatz erfindet keinen Inhaber oder Jahreswert. OpenAI-Modellgewichte,
ggml-Code und Silero besitzen getrennte Herkunftshinweise. Es werden keine
Gewichte eingebettet. Finale Target-/Feature-/Notice- und tatsächliche
Bundleprüfung sowie die übrige P11-Abnahme bleiben offen.
