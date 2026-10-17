# VAD: Fehlerstatus und vollständige Ergebnisgrenze

Stand: 17.10.2026, begrenzter Fix gegen `6beba48`. Kein Produkt-Gate und keine
Änderung der VAD-Scores bei erfolgreichem Lauf.

## Fehler und Korrektur

Der gepinnte VAD-Loop brach bei fehlgeschlagener Graphberechnung ab, gab nach
Scheduler-Cleanup aber trotzdem `true` zurück. `probs.resize(n_chunks)` ließ
partielle oder alte Werte stehen. Ein Graphallokationsfehler lieferte bereits
`false`, behielt aber Werte und bereinigte den Scheduler nicht ausdrücklich.

Die Korrektur merkt den Compute-Erfolg, erhält das gemeinsame Timing/Cleanup
und gibt bei Fehler `false` mit leerer Wahrscheinlichkeitsliste zurück. Auch
Allokationsfehler leeren die Liste und resetten den Scheduler. Der tatsächliche
Compute-Helper resetet schon bei Fehler; ein weiterer Reset ist idempotent.
Der sichere Rustwrapper bildet `false` auf `Err` ab. Aufrufer müssen diesen
Fehler weiter als Fehler behandeln, niemals als negative Sprachentscheidung.

CPP-SHA256 nach beiden lokalen Patches: `fc7f40c5d5022f45920164bfe8261de110d98a4fb88fd1ee97ef526ab00e682e`.
Nur die VAD-Fehlerzweige sind gegenüber `6beba48` geändert; SOT-Korrektur,
ABI, Decoderparameter, Modell und erfolgreiche Scoreberechnung bleiben gleich.

## Ausgeführte Regression

```sh
/usr/bin/python3 -m unittest discover -s tests/quality -p test_vad_failure.py -v
```

Der Test extrahiert die tatsächliche öffentliche C++-Funktion unverändert aus
der Vendorquelle und kompiliert sie mit `clang++ -std=c++17 -Wall -Wextra -Werror`.
Nur die Backend-/Systemgrenzen sind Doubles. Sie injizieren Computefehler im
ersten und mittleren Frame sowie Allokationsfehler nach einem positiven Lauf.
Der Compute-Double resetet wie der echte Helper bei Fehler selbst; dadurch
läuft der tatsächliche doppelte Cleanup-/Recoverypfad. Allokation modelliert
einen möglichen partiell belegten Scheduler konservativ. Der Test prüft false,
keine alten/partiellen Wahrscheinlichkeiten, bereinigten Scheduler und einen
vollständigen unabhängigen Folgejob. Ein unverändert erfolgreicher Kontrolllauf
besteht ebenfalls.

Gegen den unveränderten Gitblob `6beba48:vendor/whisper-rs-sys/whisper.cpp/src/whisper.cpp`
waren drei Tests rot und ein Kontrolltest grün. Nach dem Fix bestehen vier Tests.
Nach Anpassung des Compute-Doubles wurde derselbe unveränderte Gitblob erneut
in einer Tempdatei getestet: weiterhin drei Fehler, keine Harnessfehler. Die
Vendorquelle wurde dabei nicht umgeschaltet. Baseline-Funktionshash:
`b855707512169aee36bd994a060de6b5a91c19d5edea6157b40f8591c13086d2`.
Aktueller Funktionshash: `2de080c8e23c5399560196017372d20e28dab97f1dd2072b1094544aedb27111`.

Das ist Fault-Injection am echten Funktionskontrollfluss mit Backenddoubles,
keine behauptete reale GGML-/Hardwarefehlerinjektion oder VAD-Qualitätsabnahme.
Nichtleere, endliche und begrenzte Inputs bleiben Verantwortung des Aufrufers.

## Erfolgspfad und Buildnachweis

Nach getrenntem Clean des Release- und Debug-sys-Pakets wurde neu kompiliert.
Die aktive Release-OUT_DIR-CPP-Kopie entspricht dem obigen Hash. Dieselbe
versionierte VAD-Probe lief anschließend je 42-mal auf raw und peak-normalized,
Kennung `6beba48+vad-failure-fc7f40c5`. Alle 84 API-Läufe meldeten keinen expliziten
Fehler. Sämtliche Scorevektoren, Gains, Eingangs-/Ausgangsframes und IDs stimmen
exakt mit den vorherigen variantengleichen JSON-Werten überein.

Die Probe trägt weiterhin konservative `untrusted`-/`undecided`-Metadaten aus
dem vorherigen Experiment; die historische Upstream-Fehlerwarnung im Bericht
wurde nicht rückwirkend geändert. Dieser Fix schließt die hier getesteten
Fehlerzweige, ist aber keine pauschale Zusicherung über jeden Backendfehler.
Produktintegration, vollständige Ressourcen-/Abbruchprüfung und P05 bleiben offen.

Vollständiger Check und npm test bestehen, einschließlich 41 Python-Tests.

## Lokale Ergebnisdateien

| Datei | SHA256 |
|---|---|
| vad-calibration-v2-raw-failure-fixed.json | `3e186bcc06ed9bdb6a92369b3d606fd899be9cccb2ac95a1f1e5bd2d5ec7df7e` |
| vad-calibration-v2-peak-failure-fixed.json | `be58eb5271b778268915ffc2565559faad64b308f8fa1aff4da845bb701d030e` |
