# Unabhängiger Readback: 256-MiB-Cache in der echten Runtime

**Der integrierte RAM-Effekt ist belegt.** Gegenüber der FP16-Baseline v3 sinkt an den 13 gleichen warmen Checkpoints der mediane RSS von 1413857280 auf 860913664 Bytes: **552943616 Bytes bzw. rund 39,1 % weniger**. Der mediane wiederverwendbare MLX-Cache sinkt um 1070603348 Bytes. Aktive MLX-Allokationen und der warme Peak aktiver Bytes bleiben gleich.

Alle **198 Ablauf-, Bindungs- und Messprüfungen** sind grün. Es gibt ein konkretes **Dokumentations-Finding**: der historische Diagnosetext behauptet weiterhin keine Cacheänderung. Deshalb ist `allExecutionAndMemoryChecksPassed=true` und `ramEvidenceSufficient=true`, aber `allChecksPassed=false`. `qualified=false`, `fullSpeedQualified=false`, `fullQualityQualified=false` und `fullProductQualified=false` bleiben bestehen. Die [vollständige JSON-Rechnung](/Users/hoppworks/projects/everlastChallenge-mlx/artifacts/implementation/ram-budget-v1/cache-runtime-readback.json) enthält alle gleichphasigen Vergleiche und Einzelprüfungen.

## Gleicher Ablauf, getrennte Speichergrößen

Beide Hauptläufe verwenden dieselbe DE5-Aufnahme, dieselben Referenzbytes und denselben Workload: **drei Warmup-Sessions plus 13 weitere Sessions**, also 16 Sessionzeilen, jeweils **zwölf tatsächlich gestartete und abgeschlossene Decoderaufrufe**. Es sind acht StopComplete-Sessions einschließlich der drei Warmups, vier CancelCapture ohne Decoder und vier CancelDecoderEntry mit vollständig abgeschlossenem Decoder, dessen Ergebnis verworfen bleibt.

Baseline v3 und Kandidat haben jeweils **46 Speicherbeobachtungen** an identischen Phasen: vor/nach Laden, Ready, vor/nach jedem echten Decode, dieselben 14 Checkpoints, vor/nach dem Warm-Peak-Reset und vor/nach Finish sowie nach dem Worker-Join. Alle Identitäten, Phasenreihenfolgen, fortlaufenden Sessions, Eingangsframes, Publishcounts und Fehlerfelder wurden direkt geprüft. Keine Session wurde ausgelassen oder wiederholt. Die 13 warmen Vergleichspunkte sind ausschließlich die Checkpoints `measured-001` bis `measured-013`.

Alle folgenden Werte in **MiB = 1048576 Bytes**. „Ersparnis“ ist Baseline minus Kandidat. Die Kategorien überlappen und dürfen nicht addiert werden.

| Messgröße | FP16-Baseline v3 | 256-MiB-Kandidat | Ersparnis |
|---|---:|---:|---:|
| RSS Median, 13 warme Checkpoints | 1348.359 | 821.031 | 527.328 |
| MLX Cache Median, warme Checkpoints | 1279.197 | 258.190 | 1021.007 |
| MLX aktive Bytes, warm | 1215.041 | 1215.041 | 0.000 |
| MLX Peak aktiver Bytes nach Warmreset | 1869.901 | 1869.901 | 0.000 |
| ganzer Prozess: Maximum RSS | 1417.188 | 1319.609 | 97.578 |
| ganzer Prozess: Peak Footprint | 3313.940 | 3029.315 | 284.625 |
| RSS nach Worker-Join | 783.078 | 200.312 | 582.766 |
| MLX Cache nach Worker-Join | 2494.238 | 0.109 | 2494.128 |

Die warmen RSS-Snapshots liegen in der Baseline bei 1391394816–1417871360 Bytes und im Kandidaten bei 786300928–904445952 Bytes. Die Reduktion ist damit nicht bloß ein Unterschied beim ersten Laden. Gleichzeitig ist der ganze Prozess-Peak-Footprint nur um **298451304 Bytes** und das ganze Maximum-RSS um **102318080 Bytes** geringer. Diese kleineren Peak-Differenzen sind kein Widerspruch: sie enthalten auch das Laden und die ersten Materialisierungen.

## Gleichphasige OS-Beobachtungen

RSS ist der gespeicherte `ps`-Wert in KiB, vom ursprünglichen Messhelfer exakt in Bytes umgerechnet. Die beiden rechten Spalten sind die **unverändert gerundeten vmmap-Labels**; daraus werden keine scheinbar exakten Bytedifferenzen berechnet.

| Phase | Baseline RSS MiB | Kandidat RSS MiB | Baseline vmmap Footprint | Kandidat vmmap Footprint |
|---|---:|---:|---|---|
| before-load | 19.953 | 20.391 | 8417K | 8849K |
| after-load | 705.969 | 708.641 | 682.1M | 684.7M |
| checkpoint:warmup-003 | 1348.391 | 865.531 | 3.2G | 2.2G |
| after-warm-peak-reset | 1348.438 | 865.562 | 3.1G | 2.1G |
| checkpoint:measured-001 | 1351.234 | 862.531 | 3.2G | 2.2G |
| checkpoint:measured-007 | 1348.297 | 821.016 | 3.2G | 2.2G |
| checkpoint:measured-013 | 1327.000 | 749.875 | 3.2G | 2.2G |
| after-finish | 783.203 | 200.469 | 2.6G | 174.6M |
| after-join | 783.078 | 200.312 | 2.6G | 174.5M |

An den warmen Checkpoints meldet vmmap für die Baseline **3.1G/3.2G**, für den Kandidaten **2.1G/2.2G**. Nach Join stehen **2.6G** gegenüber **174.5M**. Das sind gerundete Momentaufnahmen, keine unabhängigen exakten Fußabdruckmessungen. Die genauen ganzen Prozesspeaks stammen separat aus `/usr/bin/time -l`.

Der explizite Warmreset setzt nur den MLX-Peakzähler auf 0. Seine Vorher-/Nachher-Belege zeigen unveränderte aktive und gecachte Bytes. Nach dem nächsten echten Decode erreichen beide Varianten denselben warmen Peak aktiver Allokationen von **1960732824 Bytes**. Weder OS-RSS noch der Peak des gesamten Prozesses werden dadurch zurückgesetzt. Baseline v2 hat nur 44 Phasen ohne diesen Reset; sie ist als ältere Einordnung bekannt, wird hier aber nicht als gleichartiger Warm-Peak-Vergleich verwendet.

## Tatsächliche Cachepolicy, Drop und Join

Der neue Lauf verwendet `MlxEngine::load_reference_encoder` über den bestehenden serialisierten InferenceService und MeasuredRuntime. Der After-load-Beleg zeigt den **wirklichen Encoder-Dtype fp16** sowie `cacheBudgetBytes=268435456`. Das Log enthält die erfolgreiche Installation desselben Budgets, den gebundenen F32-Checkpoint/CPP-Decoder und zwölf echte externe Encoderverwendungen; alle verarbeiten nach unveränderter Vorbereitung 68480 Samples aus derselben 80000-Sample-Aufnahme. Ein CLI-Label allein wurde nicht als Dtypebeweis verwendet.

Die aktiven MLX-Bytes bleiben warm exakt **1274062868**. Der Cache liegt im Kandidaten zwischen 268691620 und277732528 Bytes. Das ist ein wiederverwendbares Cachebudget und kein harter Gesamt-RAM-Deckel; eine zuletzt zurückgegebene Allokation darf laut gebundener Allocatorimplementierung kurz über der Grenze liegen.

Vor Finish bleibt das Modell warm. Der ausgewählte Guard synchronisiert und dropt das Modell, leert anschließend den wiederverwendbaren Cache und stellt das vorherige Budget wieder her; danach werden die Worker-Streams beendet. Nach Finish stehen in beiden Varianten 114688 aktive Bytes. Der Cache beträgt dann beim Kandidaten 12 Bytes, bei der Baseline 2615282968 Bytes. Nach dem bestätigten Worker-Join sind in beiden Varianten 0 aktive Bytes vorhanden; der verbliebene Cache beträgt **114700** gegenüber **2615397656 Bytes**. Die Rücknahme der verbleibenden aktiven Bytes fällt mit dem Worker-Join zusammen; die konkrete Objektzuordnung wurde hier nicht separat instrumentiert. Ein neuer Stream wurde für die Readback-Getters nicht erzeugt.

Die konkrete Schlussfolgerung ist weniger warm gehaltene und nach Join gecachte MLX-Allokation bei gleicher Arbeitslast. Das ist kein Beweis für sämtliche CPU-/Treiberallokationen, keine lange Wiederholungs-/Leakqualifikation und keine Zusage eines künftig konstanten RSS.

## Text-, Cancel- und Ownerbelege

Die gespeicherte Referenz ist SHA `d36dec2ebe1f1082ce1e143a85ab83793d413ddc8ff1e96aac1c635278050fbf`, identisch in beiden Profilen und zum vollständigen ursprünglichen CPP-Objekt für DE5. Der unveränderte [Workloadhelfer](/Users/hoppworks/projects/everlastChallenge-mlx/artifacts/mlx-qualification/cache-runtime-ram-v1/source/src-tauri/examples/runtime_resource_probe/runner.rs) vergleicht den **vollständigen veröffentlichten Stop-Text** direkt mit diesen Referenzbytes. Alle acht StopComplete-Receipts je Profil haben `referenceMatch=true`, `Local`, vorhandenen Text und keine Fehler.

Eine wichtige Beweisgrenze: Diese RAM-Probe exportiert **keine rohen Volltranskripte**. Die unabhängige Prüfung kann die in gebundenem Code tatsächlich ausgeführte Volltextgleichheit anhand ihrer Receipts und der Referenz prüfen, aber keinen zweiten Vergleich exportierter Transkriptobjekte vornehmen. Die vier CancelDecoderEntry-Aufrufe liefern bewusst keinen veröffentlichten Text. `decoderCompared=12` bedeutet vollständiger PCM-Eingangsvergleich vor dem Decode, **nicht zwölf Textvergleiche**.

Alle acht Cancel-Sessions je Profil enden mit `Cancelled` und ohne Text; CaptureCancel startet keinen Decoder, DecoderEntryCancel jeweils genau einen. Die ersten 15 Sessions bestätigen die freie Ownership durch den nächsten Start; die letzte Session bestätigt Idle und die anschließende echte Cleanup/Join. Die endgültigen Diagnosen melden 12 gestartet, 12 abgeschlossen, 12 PCM-verglichen, 0 Mismatches, keinen aktiven/armed Decoder und keinen letzten Fehler.

Beide äußeren Owner haben Exit 0, Reap und bestätigte Terminierung, keine Forced-Cleanup-, Cancel-, Watchdog- oder Cleanupbeobachtungsfehler. Die Gruppen **93767** (Baseline) und **96488** (Kandidat) wurden aktuell gezielt mit Signal 0 als abwesend bestätigt. Die in allen vmmap-Headern konsistenten Probe-PIDs sind 93768 bzw. 96489; die Header binden sie jeweils an den passenden time-Parent. Es wurde kein Stop-/Killsignal versandt.

## Quellenbindung und Power

Neuer Run-Freeze: `383b926248880ff206da4ded5adc33748ae02ba5240d2f04b02d4217486d5fdb`. Baseline-v3-Freeze: `cb7259e0219e38e2b079c5decb80f8ce0a250bcbb7054da78168e664996f8baa`. Result-, Referenz-, Fixture-, Probe-Source-, Build-/Feature- und Kontrollskriptbindungen wurden geprüft. Alle **3553** geschützten Dateizeugen des Kandidaten und **3542** der Baseline stimmen. Große Modell-/Binary-/Kerneldateien wurden dabei nicht erneut gehasht.

Der neu berechnete Produktionsquellendelta zur Baseline besteht genau aus `transcription/mlx.rs` und fünf Dateien unter `vendor/mlxrs`, welche Cache-API und ihre Dokumentation/Counters betreffen. Die eigentlichen Workload-, OS-Mess- und Runtime-Testhelfer sind identisch. Der RAM-Probe-Quellendelta fügt allein die Budgetangabe im After-load-Objekt hinzu. Es wurde kein anderer numerischer Decoder-/Inputpfad für den Vergleich eingesetzt.

Beide Läufe haben 22 Powerpunkte: Before, 20 Samples,After. Alle zeigen AC und denselben vollständigen Policytext-SHA `82b51724bc5fab9e0bbddf4401c56eaf477f32702878606a0f1ac4a9a011ff1e`; die größten Samplinglücken liegen bei 5,102945 bzw. 5,106141 Sekunden. Das sind getrennte Prozessläufe mit konsistenten Stichproben, kein Nachweis pausenlos identischer Temperatur, Last oder physischen Energieverbrauchs. Die OS-Messungen perturbieren den Workload ausdrücklich; weder Sessionzeiten noch Gesamtlaufzeiten begründen eine Speedfreigabe.

## Offenes Dokumentations-Finding

Der neue Rohreport übernimmt in `limits` weiterhin den historischen Satz **„no cache clearing or allocator limit change“**. Er ist für diesen Lauf **sachlich falsch**: echte Installation, Cachezähler und die gebundenen Quellen belegen das 256-MiB-Budget samt Cache-Clear beim Finish. Der Widerspruch liegt im Beschreibungstext; Dtype, Workload, Rohmessungen und deren Bindungen sind davon unberührt.

Die ursprüngliche Datei bleibt unverändert. Die Beschreibung soll nur für spätere separat eingefrorene Diagnosequellen korrigiert werden. Die tatsächlichen Budget-/Zähler-/Quellenbelege bestimmen die RAM-Aussage dieses Berichts. Qualitäts- und Pace-Gates sowie eine vollständige Resource-/Produktqualifikation stehen weiter aus.
