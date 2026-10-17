# P05: kontrolliertes 300-Sekunden-Supplement

Stand: 17.10.2026. Vor Inferenz festgelegte Diagnose, kein neuer Blindtest.

## Zweck und unveränderte Grenzen

Der isolierte Original-/Maskenvergleich zeigt bereits lokalen Inhaltskollaps
unter der alten starken Rauschmaske. §13 und A08 des Plans behandeln Geräusch-
fälle als dokumentierte Grenze; daraus folgt keine allgemeine Wortgarantie
unter beliebig starker Maskierung. Die Pflicht zu fünf Minuten ohne sichere
Energiepause, Vollständigkeit, begrenzten Ressourcen und Abbruch bleibt bestehen.
Ein zusätzliches kontrolliertes Signal untersucht diese eigenständige Dauerfrage.

Der alte 90er-Korpus und sein 300-s-Fall bleiben unverändert, einschließlich
Fehlern, Klassifizierung und WER-Nennern. Das Supplement ersetzt keinen alten
Abnahmefall und belegt keine neue blinde Sprachqualität. Es kann auch bei gutem
Text weder den installierten Maximaldauerablauf noch zehn warme Zeitmessungen,
Ressourcen- und Abbruchabnahme allein schließen.

## Prospektiv gewählte Rezeptur

Dieselben 21 ganzen Originalclips, Reihenfolge, 463 Referenzwörter, Speech-Gain
0,8, Xorshift32-Seed und 300 Sekunden wie beim alten konstruierten Fall. Die
Rauschamplitude wird einmal mathematisch gewählt: zehnmal der größere der
bereits eingefrorenen Energiegrenzwerte, also 10 × max(0,00005; 0,000025) =
0,0005. Keine Suche nach einem günstigen Transkriptionsergebnis. Die Rezeptur
wurde vor Erzeugung gespeichert und nach dem Erzeugen nicht angepasst.

Alle 4.800.000 PCM16-Samples und 15.000 vollständigen 20-ms-Fenster erzeugt;
kleinster gemessener RMS 0,0002655040258626123, also über beiden Grenzwerten.
Die 21 Quellclips enden weiterhin bei 287,46 Sekunden, Rest reine Grundlast.
Die Quellen wurden gegen ihre Memberhashes geprüft. Keine Wörter entfernt,
Sätze wiederholt, Referenzen repariert oder Parameter verändert.

[Parameterfreeze](../../tests/fixtures/corpus/long-control-parameters-v1.json)
bindet Manifest, WAV, Erzeugerskript, Rezeptur, Energienachweis, Binary und
unveränderte Decoderkonfiguration. Zunächst ein Referenzmodell-Dateilauf mit
vollständiger Inhaltslektüre aller 21 Blöcke: erhaltene Aussagen und Reihenfolge,
Auslassungen/Dopplungen sowie lokale Wort-/Ankerfehler getrennt dokumentieren.
Kein Ergebnis ist an dieser Stelle vorweggenommen.
