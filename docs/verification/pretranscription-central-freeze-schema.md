# Prospektiver zentraler P06-Paarstarter · externe Vorbereitung

Historisches Entwicklerwerkzeug. Der damalige Kandidat bestand die
Qualitätsprüfung nicht und darf daraus keine positive Qualitätsentscheidung
oder qualifizierende Ausführung ableiten.

## Feste Ausführungseinheit

`paced_reference_probe` führt einen warmen Modell-/Kontextprozess, genau einen
markierten DE5-Aufwärmlauf und sechs kanonische Fälle mit jeweils 30 Messungen
aus: 181 Plätze pro Block. Engine, Runtime, Decoder und Eingabetakt bleiben gleich.

Zwei getrennt geplante, unteilbare Paarrollen:

- `ref-cand`: Referenz, danach Kandidat;
- `cand-ref`: Kandidat, danach Referenz.

Jedes Paar darf ein eigenes Zeitfenster haben. Teilblöcke, fortgesetzte Plätze,
Wiederholungen oder übernommene Messungen sind ausgeschlossen. Scheitert der
erste Block, entfällt der zweite; beide Projektionen mit je 181 Plätzen bleiben
erhalten. Paare getrennt vergleichen, ABBA-L nicht mitteln. Vollständige Paare
weisen relative 10-%-Verbesserung und sechs absolute Median-/p95-Grenzen aus.
Gesamtqualifikation benötigt zusätzlich Qualität, Ressourcen, Abbruch und
Maximaldauer. Wiederholungen erhöhen nicht die sprachliche Fallabdeckung.

## Eingefrorene JSON-Konfiguration

Erforderlich sind `schemaVersion` 1, `kind` prospective-pretranscription-central-pairs-v1,
`sourceRevision` als vollständiger unveränderlicher Git-Commit und diese Bindungen:

- `binary`: target/release/examples/paced_reference_probe;
- `parameters`: Referenz-/Kandidatenobjekt der genauen zentralen Vorlagen aus
  `--print-pretrans-reference-parameters` und `--print-pretrans-candidate-parameters`,
  keine Einzelfallvorlagen;
- `manifest`: unverändertes zentrales Sechs-Fälle-Manifest;
- `inputs`: genau sechs dazu passende Audiobindungen;
- `model` und `vad`: bekannte gepinnte Hashes/Größen, Modell-ID und Nachbardateinamen;
- `sources`: eindeutige Bindungen aller rekursiv kompilierten Parameterquellen,
  von Skript/Test/Schema sowie scripts/run-paced-remainder.py und
  scripts/run-pretranscription-screening.py. Zusatzquellen werden gleich geprüft;
  keine pauschale transitive Vollständigkeit wird behauptet;
- `qualityDecision`: nachfolgend beschriebene Bindung;
- `outputDirectory`: strikt unterhalb des tatsächlichen ignorierten artifacts/.

Jede Bindung enthält `path`, `bytes`, `sha256` und `mode` als tatsächliche
`oct(stat.S_IMODE(...))`-Zeichenkette. Pfade sind begrenzt, repositoryrelativ und
ohne Traversierung. Finale Symlinks und nicht reguläre Dateien werden abgelehnt.
Quellen müssen gleichzeitig aktuellen Bytes und unveränderlicher Revision
entsprechen. Kompilierte Quellhashes müssen passen. Vor jedem Start werden
alle Vorabfingerabdrücke neu geprüft. Das erkennt gewöhnliche Änderungen,
ist aber kein Schutz gegen einen feindlichen gleichzeitigen Schreiber desselben
Benutzerkontos.

Das Parameterpaar darf sich nur in `configurationId`, `pretranscription.candidate`,
`measurement.variant` und `preprocessing.mode` unterscheiden. Alle Runtime-/
FullParams-/Modell-/Herausgebereinstellungen bleiben gleich. Verlangt werden
`audio_ctx` 0, `no_timestamps` false, `suppress_nst` false, `flash_attn` false,
TC-Publisher, ein Aufwärmlauf, 30 Messungen pro Fall, insgesamt 181 Plätze,
20-ms-Blöcke, 10-ms-Jitter, höchstens 1 ms Schlafen und 250 µs aktives Warten.
Das Binary prüft seine gesamte kompilierte Parameterdatei vor Modell-I/O.

## Gebundene externe Qualitätsvoraussetzung

Die JSON-Datei `qualityDecision` enthält:

- `kind`: pretranscription-quality-decision-v1;
- `passed`: true;
- `productionActivated`: false;
- `screeningFreeze`, `screeningCompletion`, `qualityReport`: Bindungen;
- `runtime`: exaktes zentrales Referenzobjekt;
- `candidatePretranscription`: exaktes Kandidatenobjekt.

Dieser Datensatz entsteht erst nach unabhängiger Qualitätsabnahme. Der Starter
prüft seine drei Nachweisbindungen und verlangt `completed` true, vier Blöcke
mit 100 Fällen, alle Fälle `ok` mit genau zwei Zeilen und `status` ok sowie
`validated` true. `passed` false, unvollständige/geänderte Belege und abweichende
Parameter werden abgelehnt. WER oder Semantik werden dabei nicht neu berechnet.

Der öffentliche Qualitätsbericht verlangt `schemaVersion` 1, `kind`
pretranscription-quality-screening-decision-v1, `decision.numericalScreenPassed`
true und `decision.candidateRejected` false. Fehlende, unbekannte oder ablehnende
Berichte stoppen den Lauf auch bei einer extern positiven Entscheidung.
`productionPretranscriptionEnabled`, `performanceRunStarted` und `qualified`
bleiben false, `L` bleibt null. Semantische/Ankerabnahme bleibt die externe
`decision.passed`; es wird kein zusätzliches Semantikfeld erfunden. Erfolgreiche
Inferenz ist keine Qualitätsabnahme. Der gescheiterte Kandidat bleibt abgelehnt.

## Historische Befehle

Ab Repositorywurzel:

    python3 scripts/run-pretranscription-central.py FREEZE --pair ref-cand
    python3 scripts/run-pretranscription-central.py FREEZE --pair ref-cand --execute

In einem getrennten passenden Zeitfenster:

    python3 scripts/run-pretranscription-central.py FREEZE --pair cand-ref
    python3 scripts/run-pretranscription-central.py FREEZE --pair cand-ref --execute

Standardmäßig erfolgt nur die Vorabprüfung. Ausführung verlangt ein benanntes
Paar. Das andere Paarverzeichnis darf existieren; gewähltes Verzeichnis und
Sperre werden weder ersetzt noch fortgesetzt. Intern entsteht ohne Shell:

    /usr/bin/time -l BINARY MODEL reference MANIFEST VARIANT_PARAMS NEW_RESULT

Kein Überschreiben des Laufmodus, Referenztext, expect-empty oder beliebiger
Befehl wird akzeptiert. Prozessgruppen, Überwachung, Beendigung, Einsammeln und
Beendigungsnachweis verwenden den geprüften Screening-Starter. 7200 Sekunden
sind nur eine Lebensdauergrenze, keine L-Schwelle. Vor jedem Block sind eigene
aktive Schlafunterdrückung und Netzstrombeobachtung erforderlich; danach wird
Netzstrom erneut geprüft. Durchgehender Strom oder offener Deckel werden nicht
bewiesen. Wrapper-PID und tatsächliche `processId` sind getrennte Belege.

Paarverzeichnis: 0700; Rohdaten, Metadaten, Logs und exklusive atomare Ausgaben:
0600. Der Abschluss erhält 362 Plätze, Rohbindungen, tatsächliche PIDs,
Statistiken und Fehler. Hypothesentexte bleiben privat. Fehlende, fehlerhafte
oder SHA-null-Rohbindungen machen den Block ungültig, löschen seine
Validierungsflags/Statistiken, verhindern den Folgeblock und setzen den
Paarvergleich auf null, während Rohzustände und alle 362 Plätze erhalten bleiben.
Dateiveröffentlichung ist keine Transaktion mehrerer Dateien. Eine separate
`supervisor-completion.json` bindet den Paarabschluss und belegt das Ende der
Schlafunterdrückung; ohne sie wird das Prozessarchiv nicht akzeptiert.

Jede der 181 Zeilen benötigt kanonischen Platz/Reihenfolge/Aufwärmstatus/Sitzung,
Quell-, Manifest-, Referenz-, Modell-, Parameter- und PID-Bindungen, TC-Snapshots,
Publisher-Join, Veröffentlichungs-/Stoppzeiten, tatsächliche Enginezeit,
abschließenden nicht leeren Text, zusammenhängende globale Zeitachse und
passenden lokalen Speicher. Hinzu kommen rekonstruierte veröffentlichte
Eingaben, Workerabschluss/-übernahme, monotone sitzungsübergreifende Aufträge,
unveränderter Trace nach Join und Runtime-Join.

Alle 180 Latenzen werden unabhängig berechnet: Median als Mittel des mittleren
Paars, p95 an Position 29 von 30, L als ungewichtetes Mittel der sechs Mediane.
Keine Teilstatistiken. Metadaten sind auf 512 MiB, separater Text auf 16 MiB begrenzt.

## Tatsächlicher Vorbereitungsstand

Öffentliche Quellbasis: 74ffe2d. Ausgeführt wurden nur synthetische Remote-Tests
unter Python 3.14: neun Tests für beide 181-Zeilen-Modi, zwölf Beschädigungen,
Parameterunterschiede, vollständige Qualitätsbindungen, getrennte Zeitfenster,
Sperre, fehlende Teilfortsetzung und exaktes argv. RED ging GREEN voraus.
Kein Mac-Import/-Build, keine tatsächliche Audio-/Hypothesenübertragung, Modelle,
Prozessmessung, OS-Prioritätsänderung oder ASR. Remote-Python ist nicht die
kanonische Unicode-13-Qualitätsauswertung. Keine Qualifikation/Aktivierung.
