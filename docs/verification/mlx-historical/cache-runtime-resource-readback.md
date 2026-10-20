# Unabhängiger tatsächlicher Readback: Cache256 Resource100 V2

**Alle begrenzten unabhängigen Ausführungsprüfungen bestanden.** Ein tatsächlicher Lauf,105 vollständige Operationen (5 Warmup +100 gemessen),72 Decoder-Aufrufe mit72 vollständigen PCM-Vergleichen,101 gültige OS-Checkpoints. Keine Modelle/Builds/Tests/Messungen gestartet, keine Rohdaten verändert. `qualified=false` und `resourceQualified=false` bleiben erhalten.

[Prüfcode](cache-runtime-resource-readback.py) · [vollständiger JSON-Readback](cache-runtime-resource-readback.json)

Die feste native Reihenfolge ergibt39 StopComplete,33 CancelCapture und33 CancelDecoderEntry. Alle IDs/Sessions/Statusobjekte, exakten Eingangs-/Abschlusszähler, Terminale, Feed-Framezahlen/Publikationssummen/Timingbudgets und Eigentümerübergaben wurden einzeln aus den105 Rohzeilen geprüft. Alle39 Stops bestätigen den unveränderten UTF8-Referenztext intern exakt; alle66 Cancels veröffentlichen keinen Text. DecoderEntry-Cancel lässt die tatsächliche Inferenz unter einer kontrollierten Eintrittsbarriere fertiglaufen und verwirft deren Ergebnis. Keine Aussage über native Kernelunterbrechung oder strikte Cancel-Latenz.

Der Probe exportiert keine vollständigen Transkripte. Stop-Qualität ist hier ein gebundener nativer Vergleichsreceipt mit dem zuvor identischen CPP-/F16-Quality100-Fall. Decoder-Vergleiche belegen originale vollständige PCM-Gleichheit, keine zusätzliche ASR-Qualitätsrunde. Feed-Belege sind Summen; dies ersetzt keine Pace-L-/p95-Messung.

Tatsächlich einmal FP16 geladen, F32-Checkpoint und F32-Featurebrücke zum gleichen CPP-Decoder; genau ein268435456-Byte-Cachebudget auf dem modellführenden Worker und72 externe/0 native Encoderfenster. Der vollständige gemeinsame Runtime-Baselinegegenstand einschließlich CPP-Optionen, Dtypes, Modelle, Build-/Sourceidentität stimmt mit dem tatsächlich gemessenen Pace-Cacheprofil überein. Alle87 Frozen-Dateizeugen sowie kleine Inhalte wurden geprüft. Große Modelle/Binaries/Kernel behalten bereits verifizierte Hashes und unveränderte Dateizeugen; kein erneuter großer Blobhash.

| Speicherbeobachtung | Byte |
|---|---:|
| RSS nach5 Warmups, vor Messsessions | 911474688 |
| RSS nach gemessener Session100, vor Cleanup | 804667392 |
| RSS-Differenz Warmup → Session100 | -106807296 |
| Minimum aller101 RSS-Checkpoints | 796688384 |
| Maximum aller101 RSS-Checkpoints | 911474688 |
| Prozessmaximale RSS, time-l | 1384103936 |
| Prozessmaximaler physischer Footprint, time-l | 3170683592 |

RSS ist Residenz, kein Beweis lebender Allokationen. Es wird kein beliebiger Leak-Grenzwert eingeführt. Der letzte Checkpoint liegt vor normalem Finish/Join; ein RSS-Wert nach Join wird in diesem Probe nicht exportiert. Whole-process Peaks und Warm-/Final-Checkpoint sind unterschiedliche Beobachtungen und werden nicht addiert. Das unveränderte Probe misst keine MLX-active/cache-Zähler. Die beiden vmmap-Footprintangaben bleiben gerundete OS-Strings:

- `warmup-005`: Physical footprint:         2.2G; Physical footprint (peak):  3.0G
- `measured-100`: Physical footprint:         2.2G; Physical footprint (peak):  3.0G

Normales Finish/Join bestätigt; Decoder danach nicht active/armed, started/completed/compared jeweils72, keine Mismatches/Decoder-/Cleanupfehler. Alle104 vorhergehenden Eigentümer werden durch den nächsten StartACK bestätigt; der letzte Besitzer durch das normale abschließende Join. Wrapper und Prozessgruppe wurden tatsächlich gereaped, keine ForcedCleanup-/Watchdog-/Cancel-/Powerfehler; eigene Signal0-Prüfung bestätigt ESRCH.

79 Powerpunkte mit AC und byteidentischer Policy in Before-/Sample-/Post-Reap-Rollen; maximale beobachtete Lücke5.100658s. Caffeinate ist laut gebundenem finalen Ownerjournal gewartet/reaped. Keine lückenlose physikalische/thermische Last- oder Energieaussage.

Das rohe `loadMicros` beträgt370µs und endet vor `shared::wait_ready`. Der Modellinitializer läuft asynchron auf dem permanenten Worker; der Wert misst Spawn/Setup, keine vollständige Modellladezeit. time-l real/user/sys: 393.96/11.92/10.91s; kein Coldboot- oder Joulebeweis.

V1 bleibt die gesonderte Vorbereitung mit Fehler vor ASR; V2 ist der hier geprüfte eine tatsächliche Lauf. Dieser Readback qualifiziert nur den gebundenen Lebenszyklus-/Resource100-Ausschnitt. Keine universelle Leak-/Ressourcenfreigabe; Rest94, Default/Reload, Bundle/Offline und Gesamtprodukt bleiben eigene Gates.
