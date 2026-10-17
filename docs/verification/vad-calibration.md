# P05: explorative Ganzaufnahme-VAD-Messung

Stand: 17.10.2026, SDK `3583e83`. Kein Produkt-Gate, keine P05-Abnahme.
Die bekannte native False-success-Lücke ist noch nicht korrigiert; ein
API-Erfolg ist daher ausdrücklich kein vollständiger Ausführungsnachweis.

## Pilot mit unverändertem PCM

`artifacts/corpus/vad-calibration-v2-raw.json` enthält den ersten Lauf auf allen
42 eingefrorenen Kalibrierungsfällen, CPU, ein Thread, Silero v5.1.2. Bei der
vorab festgelegten rein beschreibenden Grenze 0,5 lagen alle sechs leeren
Kontrollen darunter und alle vier kurzen Sprachfälle darüber. Drei leise
englische Fälle lagen ebenfalls darunter. Eine Schwellenregel auf diesen
Rohpegeln kann die Fälle nicht zuverlässig trennen; das wäre keine zulässige
Produktkonfiguration. Der Pilot besitzt noch keinen eingebetteten Probequellhash
und wird nicht nachträglich einem geänderten Quellstand zugeschrieben.

## Vor dem Folgelauf festgelegte zweite Variante

Die optionale Variante `peak-normalized` skaliert genau eine vollständige
Aufnahme mit dem Faktor `0.95 / max(abs(samples))`. Bei Nullsignal bleibt die
Aufnahme unverändert. Alle 42 Fälle erhalten dieselbe Regel, einschließlich
Rauschen und Raumton. Es werden keine Samples entfernt, hinzugefügt, verschoben
oder geclippt. Ein fester Zielpeak 0,95 lässt Reserve unter dem PCM-Maximum und
macht die VAD-Eingangsamplitude unabhängig vom ursprünglichen Gesamtgain.
Das garantiert keine zuverlässige Erkennung: Auch Geräusche werden angehoben
und müssen denselben Negativfallvergleich bestehen. Es ist eine getrennte
Kalibrierungsvariable, keine nachträgliche Änderung der Acceptance-Audios.

Der Default bleibt `raw-pcm16-no-gain-no-trim`. Nach Ergänzung der Source-ID und
des compile-time Probequellhashes wird raw erneut gemessen; beide Varianten
laufen seriell mit demselben Modell und Manifest. Es gibt keine ASR-Transkription.
Vorerst werden nur Scores, Frames, Gains und Kosten verglichen. Ein möglicher
Nutzen muss vor Produktintegration zusätzlich Fehlerpfade, Ressourcen, Abbruch
sowie kurze/leise Sprache in der vollständigen Abnahme bestehen.

## Tatsächlich ausgeführte Folgeläufe

Beide Varianten wurden danach mit derselben Probe gemessen, Quell-SHA256
`06ef87f7278b25872432f92ae5e35f44568a5617e466b1e895b262f0ff124e2c`,
Metadatenrevision `3583e83+vad-probe-06ef87f7`. Modell/Manifestpins entsprechen
[der Anleitung](vad-probe.md). Jeweils alle 42 Fälle und null explizite Fehler;
alle erfolgreichen API-Rückgaben bleiben wegen der Fehlerpfadlücke untrusted.

| Variante | Sprachfälle mit mindestens einem Wert >=0,5 | Leerkontrollen mit mindestens einem Wert >=0,5 | kleinstes Sprachmaximum | größtes Leerkontrollmaximum | VAD-Batch inklInput/Skalierung |
|---|---:|---:|---:|---:|---:|
| raw | 33/36 | 0/6 | 0.012030 | 0.123644 | 533.611 ms |
| peak-normalized | 36/36 | 0/6 | 0.985743 | 0.160041 | 539.398 ms |

Die Peakvariante erhält in diesem Experiment auch alle zehn leisen und alle
vier kurzen Sprachfälle. Die Input-/Outputframezahlen bleiben identisch. Ein
positiver Score hält nur die gesamte Aufnahme für spätere unveränderte
Whisper-Inferenz offen; er schneidet keine Pausen oder Wörter. Noch wurde kein
kombinierter VAD+ASR-Produktlauf ausgeführt. Die Trennung auf diesen Daten ist
Kalibrierung, keine Aussage zur separat eingefrorenen Abnahmemenge, zu jeder
0,2-s-Aufnahme oder zu beliebigem Raumton.

Die Initialisierung dauerte raw 192,060 ms und peak 187,969 ms, Freigabe 27 µs bzw. 19 µs.
Dies sind einzelne lokale Release-Beobachtungen mit CPU/einem Thread auf dem
in v3 dokumentierten Apple M5. Sie belegen weder Worst-Case-Ressourcen noch
Cancel, RSS, fünfminütige Dauer oder Echtzeitlatenz L. Die Messung begründet,
VAD mit dieser Eingangsskalierung weiter zu prüfen; sie aktiviert nichts im
Produktpfad und lockert keine Qualitätsgrenze.

Befehle wie in der Anleitung; neue Ausgabepfade `vad-calibration-v2-raw-pinned.json`
und `vad-calibration-v2-peak.json`, letzterer mit `peak-normalized`. Ein Release-
Build wurde vor beiden seriellen Modellläufen erstellt. Die eingebetteten
Quellhashes stimmen mit der tatsächlich kompilierten Probe überein.

## Lokale Artefakte

| Datei | SHA256 |
|---|---|
| vad-calibration-v2-raw.json | `eb050347def1546bb8e8c0b61062f9aadc121d1624db008638e979c0257710d0` |
| vad-calibration-v2-raw-pinned.json | `3a1c3a42dfe4855b75a69d58a1874e612c73079c6508e062868f0928e23d88a7` |
| vad-calibration-v2-peak.json | `f734f5191dd2d229c88c4fb64c034e2885aa030ff5c18bd3d559936cb38bdb61` |
