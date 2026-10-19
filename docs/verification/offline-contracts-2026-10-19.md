# Native Textaktionen und zusätzliche Offline-Verträge

Am 19.10.2026 nach ausdrücklicher Wiederaufnahme ohne Nutzeranwesenheit
bearbeitet. Produktfix-Commit `ee1a6a4`, Tooling-Commit `6842c3e`, Basis `9a2a903`;
Quellen und Logs sind im privaten
`artifacts/offline-continuation-v1/integrated-source-audit.json` gebunden.
Keine Rechteänderung, Installation oder Mikrofonaufnahme. Die installierte App
und R3 bleiben auf Produktquelle `634d6c9`; diese Quellenkorrektur ist dort
noch nicht enthalten.

## Korrigierter Snapshotfehler

War noch Text A sichtbar, während die Runtime bereits B übernommen hatte,
las der native Copy-Callback erneut den aktuellen Runtime-Text B. Eine zuvor
angestoßene Löschanfrage konnte ebenfalls später B löschen. Der unabhängige
Review benannte diesen konkreten P2-Befund.

Die nativen Copy-/Clear-Aktionen tragen jetzt den begrenzten sichtbaren Text.
Copy kopiert nach bestehender Admission genau diesen Text; Clear vergleicht
im seriellen Runtimeowner unmittelbar vor der Mutation. Bei inzwischen
verändertem Text bleibt dieser erhalten, und die Anfrage erhält einen deutschen
Fehler. Der vorhandene unbedingte Clear-Aufruf für ältere kontrollierte und
IPC-Aufrufer bleibt ausdrücklich unverändert. Neue erwartete Texte sind auf
256 KiB begrenzt; leerer Text und NUL werden vor Queueaufnahme abgewiesen.
Keine zweite Sitzungszustandsmaschine und keine Änderung des Notes-/AX-Guards.

Zwei tatsächliche Assertion-REDs vor der Korrektur: der Runtime-Test löschte
das inzwischen aktuelle B; der echte, unsichtbar erzeugte AppKit-Button lieferte
nur eine Copy-Aktion ohne sichtbaren Text. Beide sind nach der Korrektur grün.
Die Runtime-Regression enthält außerdem zwei vorher eingereihte Clear-Anfragen,
von denen nur die erste bei passendem Inhalt erfolgreich ist. Damit würde ein
Vergleich allein vor dem Einreihen nicht genügen.

## Neue Verbundtests ohne Modelle oder Gerätezugriff

- `runtime::tests::cpal_adapter_contract`: nur die Gerätekonstruktion wird durch
  `CaptureHandle::start_test_source` ersetzt. Ein wirklich publiziertes NaN
  läuft durch Ring/RecordingOwner, CaptureHandle, den produktiven CpalAudio-
  Poll-/Joinpfad und Actor bis zum Fehlerzustand. Vorheriger Text bleibt,
  kein zusätzlicher Decode oder Deliver; neuer Owner, Cancel und finaler Join
  gelingen. Kein Hardware-Abzieh- oder OS-Streamfehlernachweis. Die beim ersten
  Testlauf korrigierte Annahme einer SID nach Idle war ein Fixturefehler,
  kein Produkt-RED und keine Produktkorrektur.
- `desktop::tests::native_ack_contract`: echte native Save-/ConfirmedReset-
  Callbacks, begrenzte Settingsqueue und asynchroner ACK-Waiter. Gehaltene,
  fehlgeschlagene und geschlossene Antworten; UI-Permit und Besitzerreservation
  werden getrennt beobachtet, Wiederholung bleibt ausdrücklich. Kein GUI-
  Owner oder Store-Schreibvorgang läuft; der Fehlernachweis endet an
  `action_error`, nicht an einer sichtbaren Fensterprojektion oder OS-Freigabe.

## Scannerbefund und eng begrenzte Korrektur

Der unveränderte kanonische Secret-Scan meldete 20 Treffer in 13 generierten
`.cargo-checksum.json`-Dateien des isolierten öffentlichen Dependencyarchivs.
Der vollständig redigierte Bericht und eine Schema-/Hashklassifikation belegen
Cargo-Dateinamen und ausschließlich öffentliche SHA-256-Werte als Ursache.
Keine Zugangsdaten wurden ausgegeben oder als geheimnisfrei angenommen.

Die Ausnahme erfordert gemeinsam genau diesen Archivpfad, die Regel
`generic-api-key` und einen vollständigen 64-stelligen kleingeschriebenen
Hexwert. Keine pauschale Ausnahme für `artifacts/`, Checksummendateien anderswo
oder Anwendungsquelltext. Vier echte CLI-Tests: öffentliche Metadaten werden
akzeptiert; derselbe Wert im Quelltext, Metadaten außerhalb des Pfads und ein
anderer Credential-Shape bleiben erkannt. Mit ursprünglicher Konfiguration
scheitert genau der Metadatentest, drei Kontrollen bestehen; mit Korrektur
bestehen alle vier. Der anschließende vollständige kanonische Scan von rund
1,82 GB endet erfolgreich ohne Treffer.

## Integrierte Prüfungen und Grenzen

- `npm run check`: Format, striktes Workspace-Clippy, TypeScript, Oxlint,
  Guardrails und Vertragsprüfung bestanden.
- `npm test`: bestanden; unter anderem 223 Hosttests (zwei ausdrücklich
  ignorierte Modelltests), 60 native Unit-Tests, die eigenständigen nativen
  Verträge, 215 TypeScript-Tests, 13 Node-Tests und 204 Python-Werkzeugtests.
- `cargo test -p everlast-dictation --lib --features measurement --locked`:
  310 bestanden, zwei ignorierte Modelltests. Dies sind kontrollierte
  Verträge; keine neue Echtzeit-/ASR-Korpusqualifikation.
- Ergänzte TypeScript-Coverage: Lines 99,45 %, Statements 99,04 %,
  Branches 95,85 %, Functions 98,87 %. Dies misst den erhaltenen TypeScript-
  Code, keine AppKit-Darstellung. Der Bootstrap-Ausschlussgrund ist auf den
  tatsächlichen inerten Starttext/CSS nachgeführt.
- Abschließendes striktes Clippy mit Messfeature bestanden. Die echten
  positiven/negativen Guardrail-Fixtures einschließlich instrumentierter
  Rust-/Frontend-Coveragegrenzen bestanden ebenfalls. Abschließender
  kanonischer Secret-Scan: rund 1,83 GB, Exit 0 ohne Treffer.
- `cargo deny check`: Advisories, Bans, Lizenzen und Quellen bestanden;
  bestehende Warnungen über mehrfache transitive Versionen bleiben sichtbar.
  `npm audit --audit-level=high`: null gemeldete Schwachstellen.
