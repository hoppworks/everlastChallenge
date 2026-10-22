# P06: geprüfte Integration des zentralen Starters

Historisches Entwicklerwerkzeug. Der damalige Vorabtranskriptionskandidat
bestand die Qualitätsprüfung nicht. Diese Integration belegt weder einen
Leistungslauf noch einen eingefrorenen Messstand oder eine Produktaktivierung.

Drei Dateien wurden aus
`artifacts/p06-pretranscription-screening/p06-central-pairs-74ffe2d-r2.tar.gz`
übernommen, SHA256
`e93344f23a9493173474b24dc480a32aad19a64c99ead689be0f2b5d49869236`.
Die archivierten Abhängigkeitskopien für Restblock/Screening wurden nicht
übernommen. Der Restblockadapter behält Integration `0cc8753`. Der kombinierte
unabhängige Remote-Review samt gezieltem r2-Abschluss erfolgte zuvor; das
r2-Archiv bleibt unverändert.

Der Starter akzeptiert nur vollständige Paare Referenz→Kandidat oder umgekehrt.
Jeder Block hat einen Prozess, einen Aufwärmlauf und sechs mal 30 Messplätze.
Getrennte Zeitfenster sind zulässig, Teilfortsetzung/Wiederholung nicht.
Gebunden werden eine externe Qualitätsentscheidung und ein bekannter
öffentlicher Zahlenbericht. Fehlende/negative Qualität oder ungültige
Rohbindungen verhindern Vergleich/Fortsetzung. Bei Fehlern bleiben 362 Plätze
erhalten. Runtime, Decoder, Messbudget, Manifest und Qualitätsprüfung bleiben gleich.

## Tatsächlicher macOS-Kompatibilitätsfehler

Python 3.9.6 / Unicode 13.0.0 erzeugte zuerst **einen Fehler in neun Tests**.
`test_quality_decision_is_bound_to_complete_screen_and_this_candidate` lehnte
eine temporäre Wurzel unter `/var/...` ab, deren Kind nach `/private/var/...`
aufgelöst wurde.

Einzige Änderung gegenüber geprüftem r2: Die gewählte Wurzel wird mit
`Path(root).resolve()` normalisiert, wie bereits im Vorabcheck. Pfadbegrenzung
und Symlink-Regeln bleiben erhalten. Keine Erwartung wurde abgeschwächt;
derselbe Test bestand danach.

## Prüfung

- `/usr/bin/python3 -m unittest discover -s tests/quality -p test_pretranscription_central.py`:
  neun bestanden (0.312s), einschließlich vollständiger 181-Zeilen-Modi,
  zwölf Beschädigungen, Rohbindungsfehler, widersprüchlicher Zahlenberichte,
  getrennter Zeitfenster, Sperre, fehlender Teilfortsetzung und exaktem argv.
- `... -p test_paced_remainder.py`: 21 bestanden (0.324s), einschließlich der
  tatsächlichen eigenen macOS-Prozessgruppe mit `/usr/bin/time -l`.
- `... -p test_pretranscription_screening.py`: 17 bestanden (0.806s), gemeinsame
  Validator-/Prozessgrenzen ohne Modellinferenz oder Änderung der OS-Priorität.
- Python-3.9-`py_compile` und `git diff --check`: Exit 0.
- `node scripts/guardrails.mjs`, `node scripts/check-contracts.mjs` und
  `node scripts/secrets.mjs`: Exit 0, kein Secret-Fund.

Starter-SHA256:
`def627a92913fe29554d9e91fcd8249c30898cbcebce84626109c90246721625`.
Tests und Schema sind bytegleich zu r2; Hashes:
`09a7bc12b862b1c0ada0e22ce41c1d508001ed80b23c752de52caa79c799c6d4`
und `6a069855a969c93bd5b39abfbeba956f85f750caf02f19a8e5663fff1c4a0126`.

Kein echtes Paar, Modell, ASR, Audioreplay, App-, Bundle- oder Releasebuild
wurde ausgeführt. Keine tatsächliche Hypothesen-/Audioübertragung und kein
P05-/P06-/nativer Abnahme- oder allgemeiner Leistungsnachweis.
