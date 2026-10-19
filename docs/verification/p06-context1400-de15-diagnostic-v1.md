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

## Nächster technischer Schritt

Ein unabhängig durchgeführter enger Source-Readback hält einen kurzen
measurement-only Kombinationspilot mit Full-Prefix technisch für möglich.
Der vorhandene Kontextpatch allein erreicht aber nur `transcribe_profiled_context`,
nicht die PrefixCache-Misses. Er darf nicht als fertige Kombination ausgegeben
werden. Notwendig wären feste Kontextpolicy am Engine-/Cache-Konstruktor,
normaler Energie-/VAD-/Vorbereitungspfad vor Auswahl, Modellkontext 1500,
explizite Grenze von 1 bis 320000 vorbereiteten Frames und Kontextbindung in
Cachekey/Receipt. Kein stiller Referenzfallback und keine Änderung der
Produktionsloader oder `APP_POLICY`.

Wegen des neuen DE15-Textunterschieds zuerst ein kleiner inhaltlicher Vergleich
beider festen Modi gegen dasselbe Gold mit bestehender Normalisierung;
erst danach gegebenenfalls ein kurzer echter paced Kombinationspilot.
Kein neuer langer Qualitätslauf aus diesem Befund. Die isolierten 6–8 %
Zeitgewinn garantieren keinen Durchbruch: Im historischen Full-Prefix-Lauf
fehlen DE10 rund 8,34 % und DE15 rund 9,40 % bis zur absoluten Grenze.
Zeitanteile unterschiedlicher Aufbauten dürfen nicht einfach verrechnet werden.

Produktquelle, installierte R4-App und Reference-Konfiguration unverändert.
P06 sowie Gesamtabschluss bleiben offen.

Unabhängiger Artefaktreadback ohne Befund: Bindungen, Kontextdelta, 16 Aufrufe
mit vier Warmups, Identitäten/Join, Prozessintervalle, Mediane und Textabweichung
bestätigt. Keine Qualitäts- oder Leistungsfreigabe daraus abgeleitet.
