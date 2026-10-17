# P05: erste Kalibrierung der vollständigen Referenzen

Stand: 17.10.2026, Quellrevision `6139e55`, Konfiguration
[`complete-recording-reference-v1`](../../tests/fixtures/corpus/reference-parameters-v1.json).
Beide Modelle liefen nacheinander auf demselben dokumentierten Apple-M5-Gerät.
Es wurden ausschließlich die 34 Fälle mit `split == calibration` verwendet.
Die Abnahmemenge wurde nicht transkribiert. Parameter blieben unverändert.

Vollständiger eingefrorener Manifest-SHA256:
`734199a55fe70bd0cf0dbc7df2fb3ea7afc7de14ed4daf20aa3d0a5672f0c60b`.
Das abgeleitete lokale Manifest `artifacts/corpus/calibration-v1.json` enthält
zusätzlich Herkunft und Filter; SHA256:
`39d1029d375a6f639fc9b7c44c3c897252f7baee6b081046f789f77f3c5365b6`.

## Ergebnis

Alle 34 Jobs je Modell lieferten technisch erfolgreiche Ergebnisse. Vierzehn
davon waren bei **beiden** Modellen leer: sämtliche zehn leisen Varianten sowie
vier unveränderte englische FLEURS-Aufnahmen. Diese Leerausgaben zählen als
vollständige Wortauslassungen, nicht als erfolgreiche Erkennung.

| Modell | DE sauber, 218 Wörter | EN sauber, 207 Wörter | DE leise, 121 Wörter | EN leise, 114 Wörter |
|---|---:|---:|---:|---:|
| large-v3-turbo-q5_0 | 10 Fehler / 4,59 % | 94 / 45,41 % | 121 / 100 % | 114 / 100 % |
| small-q5_1 | 16 Fehler / 7,34 % | 98 / 47,34 % | 121 / 100 % | 114 / 100 % |

Dies sind einmalige Kalibrierungswerte, keine Abnahmewerte oder Modellwahl.
Der Dateibackend-Lauf misst keine Echtzeitlatenz L. Ladebeobachtungen waren
1336 ms beziehungsweise 557 ms; sie sind keine wiederholte Kaltstartabnahme.
Stille-/Geräuschfälle gehören nicht zu diesen 34 Fällen; die entsprechende
Werkzeugmeldung mit null unerwünschten Ausgaben beweist deshalb nichts über
Halluzinationsschutz.

## Diagnose vor Parameteränderung

Die feste Startschwelle der Energieanalyse beträgt RMS 0,01 je 320 Samples.
Genau die 14 leeren Fälle unterschreiten diese Schwelle in **jedem** Fenster.
Kein durchgelassener Fall blieb im Referenzmodell leer. Der Produktpfad gibt bei
fehlendem Energiebereich vor dem Decoder einen leeren Text zurück. Somit
verhindert die vorläufige Vorfilterung hier die Inferenz; ein Vergleich allein
der Modelle würde die gemeinsame Ursache übersehen.

Bei den vier unveränderten englischen Fällen wurde zusätzlich das gepinnte
Float32-Original erneut geprüft und mit dem PCM16-Fixture verglichen:

| Fall | maximale 20-ms-RMS im Original |
|---|---:|
| fleurs-en-dev-1511 | 0,0060306 |
| fleurs-en-dev-1512 | 0,0020787 |
| fleurs-en-dev-1514 | 0,0026133 |
| fleurs-en-dev-1517 | 0,0018221 |

Die Framezahlen stimmen überein, der maximale Konvertierungsfehler beträgt
jeweils höchstens einen halben PCM16-Schritt (1/65536). Der niedrige Pegel liegt
bereits in der Quelle vor. Eine Veränderung der Konvertierung ist dafür nicht
begründet. Die Energieparameter müssen auf der Kalibrierungsmenge überarbeitet
und zusammen mit Stille-/Geräuschschutz neu eingefroren werden. Eine solche
Korrektur und erneute Messung sind noch nicht erfolgt. Abnahmedaten bleiben
bis dahin unbenutzt.

## Lokale Nachweise

Rohtexte und jobweise Messmetadaten bleiben unter `artifacts/corpus` und werden
nicht eingecheckt. Die Probe erhält die Revision über
`EVERLAST_BENCHMARK_REVISION=6139e55`. Reproduktionsweg und CLI stehen im
[Korpus-Leitfaden](../../tests/fixtures/corpus/README.md).

| Datei unter artifacts/corpus | SHA256 |
|---|---|
| calibration-reference-v1-results.json | `db2687394239daa0fc1c7c2e75c1bee40c058ba7fb3d355d3a0cd2f04e4e2022` |
| calibration-reference-v1-results.json.metadata.json | `a1ac37010830cc6c664d65007aabf41b78796b94bb93d5cd617bad415911322b` |
| calibration-reference-v1-report.json | `e4193588d5ad57e4adfe3d7ac6bacc215a4aaaa0bf07e2eae5372c52e76aced6` |
| calibration-small-v1-results.json | `562d5fbc1087b24974849218f858bdf61afd2759e8a0018d5cfe87048acc4113` |
| calibration-small-v1-results.json.metadata.json | `8b1c5296079178a50c53a347ffafaf00a2d19d7e9201378ee6191b961d77fdfe` |
| calibration-small-v1-report.json | `1844330717388997910f6b774f535e4436adb2b1d1ddd4b986758eb8c16ca427` |

P05 bleibt offen, einschließlich aller im Manifest geführten Abdeckungslücken
und der Echtzeit-, Ressourcen-, Abbruch- und nativen Abnahmen.
