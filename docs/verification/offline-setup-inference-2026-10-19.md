# A20: echte Einrichtung und Inferenz bei verweigertem Downloadtransport

Der neue, ausdrücklich ignorierte Fixturetest in `src-tauri/src/setup.rs`
schließt die bisher fehlende Verbindung zwischen Modellcache, SetupService
und tatsächlichem Inferenzworker. Er verändert ausschließlich `cfg(test)`;
Produktparameter, Benutzer-Cache, Installation und Rechte bleiben unverändert.
[Befehle, Eingaben und Dateibindungen](offline-setup-inference-2026-10-19.json).

## Tatsächlicher Ablauf

Die zwei fest gepinnten Modellfiles wurden aus dem vorhandenen Cache nur
gelesen und in ein eigenes privates temporäres Verzeichnis kopiert. Keine
Hardlinks oder Symlinks zum Benutzer-Cache. Die Kopien wurden auf Größe und
SHA256 geprüft. Der kontrollierte Backendadapter delegiert Cacheinspektion
und Modellladen an den produktiven `NativeBackend`; ausschließlich
`download` zählt den Versuch und verweigert ihn mit `ModelError::Network`.

- Vollständiger Cache: echter SetupService erreicht `Ready`, lädt die native
  Reference-Inferenz einmal und übergibt den warmen Dienst genau einmal.
  Downloadversuche: **0**.
- Der zurückgegebene tatsächliche InferenceService verarbeitet den öffentlichen
  deutschen FLEURS-Satz1517: **168.960 PCM-Frames**, Session73, Generation1,
  Job81, jeweils Sample-/Timelinebereich `0..168960`.
- Tatsächlicher Text: **134 UTF-8-Bytes**, SHA256
  `161df165fab1271854e8ff49cb989e6f2f9d9192b3c02a06e7384de01534f214`.
  Der feste Referenzhash stimmt; Text wird nicht ausgegeben oder gespeichert.
- Fehlender eigener Cache ohne Downloadauftrag: `Missing`, kein fertiger
  Inferenzdienst und weiterhin kein Downloadversuch.
- Expliziter Downloadauftrag am fehlenden Cache: genau ein abgefangener
  Versuch, anschließend `Failed/Network`, keine zusätzliche Modellladung
  und kein fertiger Dienst. Diese Gegenprobe bestätigt die aktive Sperre.
- Der echte Inferenzdienst und beide Setupdienste wurden geordnet beendet;
  der eigene temporäre Cache wird durch RAII entfernt.

## Ausführbares Feedback und Grenzen

Der gezielte Test lief tatsächlich: **1 passed, 0 failed, 0 ignored**;
237 andere Tests waren herausgefiltert. Standardfeatures, Testprofil mit
Debug-Informationen. Harnessdauer60,66s einschließlich mehrerer vollständiger
Modellprüfsummen und eines Inferenzaufrufs; keine Aufnahme- oder Latenzmessung.
Fokussiertes Clippy für Library und Tests mit `-D warnings` bestand.
Unabhängiger kombinierter Review des Testseams ohne Befund.

Die bestehende Fähigkeit wird erstmals an dieser Verbindung belegt; es wird
kein vorheriger Produktfehler oder künstlicher RED-Lauf behauptet. Der Test
bleibt in der regulären Suite ignoriert und verlangt bei expliziter Ausführung
alle drei Modell-/Fixturepfade; fehlende Dateien ergeben keinen stillen Pass.
Quellsnapshot, tatsächliche Testbinärdatei, Cargo-Fingerprint und Logs sind
im privaten Artefaktordner erhalten und gehasht.

Dies ist ein Nachweis **bei verweigertem Produkt-Downloadtransport**, keine
prozessweite oder globale Betriebssystem-Netzsperre. Native Modellinspektion,
Ladung und lokale Inferenz sind real; ein Modell-/Setupfehler wird nicht durch
einen EmptyDecoder oder behauptete Bereitschaft ersetzt. GUI, Mikrofon,
Hotkey, Einfügen, voller Qualitätskorpus und Performance sind nicht Bestandteil
dieses Tests. A20s Setup-/Cache-/Inferenzgrenze ist damit belegt; P08 und die
Gesamtaufgabe werden dadurch nicht automatisch abgeschlossen.
