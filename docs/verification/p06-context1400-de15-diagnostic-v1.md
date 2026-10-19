# Kontext 1400 am bekannten deutschen 15-Sekunden-Fall

Begrenzte Datei-Diagnose am 19.10.2026, kein Korpusvergleich und keine
Leistungsfreigabe. [Strukturierter Beleg](p06-context1400-de15-diagnostic-v1.json).

## Ausführung

Derselbe unveränderte, eingefrorene Diagnosebinary wie beim
[DE10-Piloten](p06-context1400-de10-diagnostic-v1.md). Binary, beide
Parameterdateien, ASR/VAD und öffentliche WAV vorab gegen bekannte Pins
geprüft. Fall `fleurs-de-test-1668`, bereits im fehlgeschlagenen ersten
Leistungspaar exponiert. WAV-SHA256
`27e005fc4ffab77389bda81861250b6a3616fb2e686e65ef366314bad791abf6`.

Prospektiv festgelegt: Referenz, 1400, 1400, Referenz. Jeder Prozess ein Warmup
und drei Messungen, insgesamt 16 Aufrufe. Alle Prozesse Exit 0, gültige
Profilidentitäten und erfolgreicher Workerjoin. Tatsächliche PIDs sowie
Start-/Endzeitstempel in UTC und monotone Prozessintervalle festgehalten;
Root-PIDs stimmen mit Probereports überein, Intervalle überlappen nicht.
Netzstrom vor/nach dem Pilot. Keine Wiederholungen oder entfernten Werte.

## Ergebnisse

| Block | Kontext | NativeFull-Median | EngineTotal-Median |
|---|---:|---:|---:|
| 1 | Referenz 0 / Modell 1500 | 1008,284 ms | 1043,279 ms |
| 2 | 1400 | 947,412 ms | 979,280 ms |
| 3 | 1400 | 952,285 ms | 985,297 ms |
| 4 | Referenz 0 / Modell 1500 | 1010,186 ms | 1043,869 ms |

NativeFull sinkt deskriptiv um 6,037 % beziehungsweise 5,732 %, EngineTotal um
6,134 % beziehungsweise 5,611 %. EngineTotal ist kein Stop-bis-Text-Wert:
keine tatsächliche Capture-/Actor-Finalisierung oder Übergabe gemessen.
Keine eigenständige reproduzierbare 10-%-Freigabe und kein L-Nachweis.

**Der Text ändert sich reproduzierbar.** Alle acht Referenzausgaben haben
Hash `767a99e37ff3d574fb3c12d7bb90b120d499d5355bcbab7fa94f208e0fea3340`,
alle acht 1400-Ausgaben Hash
`ceaf7baac48dbdffe8a67c6254720f9606600cb6ff3dc456291bac844d0a2439`.
Der Diagnosebinary exportiert ausschließlich Texthashes, keine Hypothesen.
Daraus folgt weder bessere noch schlechtere Erkennung; insbesondere ist
Qualitätsneutralität nicht belegt. Der DE10-Fall mit gleichen Hashes ist
kein Ersatz für diesen Unterschied und keine allgemeine Qualitätsfreigabe.

## Korrektur: vorhandene Inhalts- und Kombinationsbelege wiedergefunden

Die oben beschriebene Ungewissheit war zum Zeitpunkt dieses Readbacks eine
Wissenslücke der Session. Der [frühere Futilitybericht](p06-context1400-futility-2026-10-19.md)
enthält bereits einen tatsächlichen Single-/FullPrefix-Kombinationspilot und
seine Offline-Inhaltsprüfung. Diese Belege wurden bei der Planung übersehen.
Der neue Dateipilot war deshalb eine vermeidbare Wiederholung; seine Rohwerte
bleiben unverändert erhalten und werden nicht als notwendige neue Abnahme ausgegeben.

Der [Offline-Hashtransfer](p06-context1400-existing-content-bridge-v1.json)
prüft alle zwölf damaligen Eingabepins erneut und bindet jede der 16 neuen
DE15-Ausgaben bytegenau über ihren Texthash an die vorhandenen Hypothesen.
Unverändertes eingefrorenes quality.py mit Unicode 13 bestätigt gegen dasselbe
Gold: Referenz 2/24 Fehler = 8,3333 %, Kandidat 6/24 = 25 %. Vier zusätzliche
normalisierte Wörter, keine bloße Interpunktion. **Die DE15-Qualität ist in
diesem konkreten Fall schlechter.** Keine Aussage über gesamte Korpus-WER.

Die zuvor vorgeschlagene erneute Inhaltsausgabe und der anschließende
Kombinationspilot sind damit überholt. Vorbereitete separate Ausgabeerweiterung
vor Build/ASR verworfen, generierte Sourcekopie entfernt. Keine weitere Messung
der unveränderten Variante aus diesem Befund. Der vorhandene Kombinationspilot
verfehlte bereits die beobachtete Stop-bis-Text-Grenze. Keine Aktivierung;
Leistungsproblem bleibt technisch offen und verlangt einen anderen begründeten
Ansatz oder eine ausdrückliche Produktentscheidung.

Produktquelle, installierte R4-App und Reference-Konfiguration unverändert.
P06 sowie Gesamtabschluss bleiben offen.

Unabhängiger Artefaktreadback ohne Befund: Bindungen, Kontextdelta, 16 Aufrufe
mit vier Warmups, Identitäten/Join, Prozessintervalle, Mediane und Textabweichung
bestätigt. Keine Qualitäts- oder Leistungsfreigabe daraus abgeleitet.
