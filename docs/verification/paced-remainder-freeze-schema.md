# Prospektiver Freeze für die feste 94er-Restmessung

`python3 scripts/run-paced-remainder.py RUNFREEZE_JSON` ist ein enges Entwicklerwerkzeug,
keine allgemeine Jobplattform. Es startet ausschließlich die bekannte
`paced_runtime_probe`-CLI seriell mit einem Warmup und zehn Messungen pro Fall.
Es verändert weder die feste Auswahl noch Parameter, Referenzen oder Produktcode.
Ein echter Lauf benötigt einen separat geprüften und vorab committeten Freeze.

## Schema v1

Der Freeze ist ein JSON-Objekt mit diesen Feldern:

```json
{
  "schemaVersion": 1,
  "kind": "prospective-paced-remainder-v1",
  "sourceRevision": "FULL_40_HEX_GIT_COMMIT",
  "binary": {"path":"target/release/examples/paced_runtime_probe", "sha256":"SHA256", "bytes":1, "mode":"0o755"},
  "parameters": {"path":"tests/fixtures/corpus/NEW_PARAMETERS.json", "sha256":"SHA256", "bytes":1, "mode":"0o644"},
  "selection": {"path":"tests/fixtures/corpus/remainder-timing-selection-v1.json", "sha256":"dfb0f38743cd2a67071b327eb0da7251b0655e626edbad90fe04da88b7cd3e2c", "bytes":66497, "mode":"0o644"},
  "sources": [],
  "model": {"path":"artifacts/models/ggml-large-v3-turbo-q5_0.bin", "id":"large-v3-turbo-q5_0", "sha256":"394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2", "bytes":574041195, "mode":"0o644"},
  "vad": {"path":"artifacts/models/ggml-silero-v5.1.2.bin", "sha256":"29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf", "bytes":885098, "mode":"0o600"},
  "inputs": [],
  "outputDirectory": "artifacts/NEW_PARENT/NEW_RUN",
  "publisherPolicy": "timeConstrained",
  "caseTimeoutSeconds": 7200
}
```

Das Beispiel ist kein ausführbarer Freeze: echte Größen/Hashes, Quellen und
94 Eingabebindungen müssen ergänzt werden. `mode` ist der Dateimodus.
Alle Pfade sind repositoryrelativ; das neue Ausgabeziel liegt unter `artifacts/`
und sein Elternverzeichnis muss bereits existieren. Symlinks als gebundene letzte
Dateikomponente sowie auf Ziele außerhalb des Repositorys werden abgelehnt.

`sources` enthält mindestens die deduplizierten `filename`/`sha256`-Pins aus dem
vollständigen kompilierten Parameterdokument plus Script, zugehörige Unittests
und dieses Schemadokument. Zusätzliche eindeutig gebundene Quellen sind erlaubt;
sie durchlaufen dieselbe aktuelle/immutable Prüfung und Dateikontrolle vor jedem
Start. Das ist keine Behauptung einer vollständigen transitiven Quellenclosure.
Jeder Eintrag verwendet die Bindingform
`{path,sha256,bytes,mode}` und muss aktuell sowie in `sourceRevision` identisch
sein. Binary und Parameter werden separat gebunden. Das Parameterdokument muss
die Release-TC-Variante `paced-full-recording-native-timestamps-rtf-tc-owned-v6`
ausweisen; der native Probe prüft vor Modellinitialisierung zusätzlich dessen
vollständige Gleichheit mit seinem kompilierten Template.

`inputs` enthält genau die 94 verschiedenen WAV-Pfade derselben unveränderten
Auswahl. Jeder Input hat die Bindingform. Alle Bytes/Hashes, PCM16-Mono-16k-
Header, Framezahlen, historischen Quellenmanifeste und unabhängigen UTF-8-
Referenzhashes werden vor dem ersten Start geprüft. Vor jedem weiteren Start
werden Dateidentität, Größe, Modus, mtime und ctime erneut verglichen; Änderung
sperrt den Lauf statt große Modellhashes 94-mal zu lesen. Dies ist kein Schutz
gegen einen böswilligen gleichzeitigen lokalen Schreiber.

## Ausführung und Fehlererhalt

Der Executor erzeugt `0700` für das neue Verzeichnis. Pro Fall wird ausschließlich
folgende Argumentliste erzeugt, ohne Shell oder frei wählbare Kommandos:

```text
/usr/bin/time -l BINARY MODEL reference WAV de|en PARAMETERS NEW_RESULTS 10 [--expect-empty]
```

Local erhält ausdrücklich keinen Exact-Reference-Pfad. Empty erhält zwingend
`--expect-empty`. Der Popen-PID wird als `wrapperPid` bezeichnet; die tatsächliche
Probe-ID stammt getrennt aus `metadata.processId`. Direkt nach `Popen` wird nur
eine geflushte JSON-Fortschrittszeile mit `caseId`, `wrapperPid` und `startedUtc`
ausgegeben, ohne Kommandos oder Transkripte. Jede time-l-Ausgabe bleibt
privat im kombinierten `0600`-Log; sie misst Prozessressourcen einschließlich
der langen Inferenz, weder isolierten Live-Heap noch einen erfundenen Heapgrenzwert.

Fallziele sind `NNN-CASEID.json`, `.json.metadata.json`, `.json.command.log` und
`.json.completion.json`. Die native Probe behält ihre elf IDs `warmup` und
`measured-001` bis `measured-010`. Der Executor bindet sie zusätzlich eindeutig
als `CASEID::warmup`/`CASEID::measured-NNN`. Completiondateien werden atomar mit
`0600` und ohne Ersetzen veröffentlicht. Die Rohdateien bleiben auch bei Fehlern
unverändert. Es gibt keine Retryfunktion.

Erfolg verlangt alle elf Rows samt eindeutiger Identität, tatsächlichem Outcome,
Session-/Capture-Uhr, Frame-/Hash-/Parameter-/Modellbindungen, allen 20-ms-
Publikationen und unverändertem 10-ms-Budget, Stop-/Terminal-/Textzeiten,
Engine-RTF-Rechnung und 10/10 Timingcoverage sowie Runtime- und Publisherjoin.
TC requested/configured/beforeFeed/afterFeed/post müssen vorhanden und konsistent
sein; beobachtete Default-/Basicpolicy- oder Tickabweichungen sind Fehler.
Der anfängliche Pre-Policy-Snapshot ist davon ausgenommen. Rohe Policy-/CPU-
Abfragen belegen keine kontinuierliche Abwesenheit einer Demotion.

`caseTimeoutSeconds` darf entfallen oder genau 7200 sein. Dies ist ausschließlich
ein Runner-Watchdog, kein L-, RTF- oder Qualitätskriterium. Wrapper und Kind starten
in einer neuen eigenen Prozessgruppe. Bei Watchdog/Signal wird nur diese Gruppe
beendet; der Executor wartet auf den Wrapper und bestätigt das Verschwinden der
Gruppe. Unbestätigte Terminierung bleibt Fehler. Nach dem ersten abnormalen Exit,
Launch-, Report-, Setup-, Policy-, Outcome-, Timing- oder Cleanupfehler werden alle
folgenden Fälle `unavailable`; der globale Abschluss erhält stets die 94 Fallslots
und 1034 Zeilen. Keine Statistik über eine erfolgreiche Teilmenge. Kann eine
Datei wegen eines I/O-Fehlers nicht veröffentlicht werden, endet der Executor
explizit mit Fehler; vorhandene Rohdateien werden nicht ersetzt.

Metadaten werden mit maximal **512 MiB** gelesen: ein 300-s-Fall hat
11 × 15000 = 165000 Publikationen; das Budget erlaubt etwa 3 KiB pro Record plus
Header/Policyinformationen. Separate Texte bleiben auf **16 MiB** begrenzt.
Übergröße wird vor dem Lesen anhand des offenen Dateideskriptors abgewiesen.

Eine private `freeze.json` bewahrt die exakten Bytes des ursprünglichen Runfreeze.
Der private `reference-manifest.json` enthält genau 94 unveränderte Referenzen,
keine ASR-Hypothesen. Spätere WER-Auswertung kann je Runindex höchstens 94 Fälle
an den Unicode-13-Evaluator geben; die 1034 Runzeilen dürfen nicht als ein Corpus
an dessen 500-Fälle-Grenze vorbei übergeben werden. Der Executor normalisiert
keine Referenz und führt selbst keine WER-/Ankerprüfung durch. Er behauptet keine
L-/RTF-/P05-/P06-, Mikrofon- oder installierte Appqualifikation.

### Explizite Vorabtranskriptions-Messvarianten

Neben der unveränderten v6-Konfiguration akzeptiert der Executor ausschließlich
die kompilierten Einzelaufnahme-IDs `p06-pretranscription-reference-v1` und
`p06-pretranscription-candidate-v1`. Publisher-, Timing-, Input-, Modell- und
Cleanupprüfungen bleiben identisch. Der Parameterbeleg muss weiterhin mit dem
vollständigen tatsächlichen Metadaten-Dokument übereinstimmen; das Probe-Binary
akzeptiert nur seinen exakten kompilierten Parametersatz.

Für diese beiden Varianten muss zusätzlich
`scripts/run-pretranscription-screening.py` in den eindeutigen, aktuellen und
gegen dieselbe immutable Source-Revision geprüften Sourcebindungen stehen.
Sein reiner Tracevalidator wird wiederverwendet: jede der elf Zeilen (Warmup
eingeschlossen) benötigt begrenzte Session-/Generation-/Jobidentität, lückenlose
globale Frameabdeckung, passende lokale Storagebereiche, vollständige
InputProofs aus den tatsächlich bereits publizierten Frames, monotone
Worker-/Settlement-/Adoptionskanten und endgültige Audiofreigabe. Job-IDs dürfen
zwischen Sessions nicht zurückspringen oder wiederverwendet werden. Der
gemeldete Varianten-Bool muss zur Parameter-ID passen, ein Audio-Context-Mix
wird abgewiesen. Nach dem Owner-Join muss der letzte Trace unverändert und
fehlerfrei vorliegen. Ein fehlender oder widersprüchlicher Nachweis stoppt die
Folgefälle unter Beibehaltung aller 94 Slots und 1034 Zeilen. Dies ist ein
diskreter Nachweis, keine kontinuierliche Scheduler- oder Ressourcenbehauptung.
