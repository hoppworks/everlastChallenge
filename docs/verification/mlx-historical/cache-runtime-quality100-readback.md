# Unabhängiger Quality100-Readback: integrierter FP16-Cache256

**Der feste 100er-Qualitätslauf ist bestanden.** Alle **332 unabhängigen Prüfungen** sind grün. Alle 100 vollständigen Ausgabeobjekte sind exakt identisch zum zuvor akzeptierten FP16-Lauf. Gegenüber der unveränderten ursprünglichen CPP-Referenz sind 98 Objekte identisch; ausschließlich die zwei bereits früher akzeptierten vollständigen Änderungspaare bleiben übrig. Keine neue semantische Freigabe und keine Modellwiederholung wurden vorgenommen.

Die [Acceptance-Datei](/Users/hoppworks/projects/everlastChallenge-mlx/artifacts/mlx-qualification/reference-encoder-f16-cache256-v2/quality100/acceptance-readback.json) ist exklusiv erstellt. Sie setzt `quality100Passed=true`, `allExecutionChecksPassed=true`, `allQualityGroupsPassed=true` und `semanticReviewComplete=true`. Der [unabhängige JSON-Report](/Users/hoppworks/projects/everlastChallenge-mlx/artifacts/implementation/ram-budget-v1/cache-runtime-quality100-readback.json) setzt `allIndependentChecksPassed=true` und ist per SHA in der Acceptance gebunden. Alle Produkt-/Pace-Freigaben bleiben false.

## Vollständigkeit und tatsächliche Ausführung

Der gespeicherte Output und die 100 Metadatenzeilen enthalten exakt die 100 eindeutigen Manifest-IDs in kanonischer Reihenfolge. Alle Status-/ExecutionStatus-Felder sind `ok`, alle fallweisen Fehler sind null. Load-, Service- und Cleanup-Fehler sind ebenfalls null. Der einzelne Modellowner **97258** hat tatsächlichen Exit 0, bestätigten Reap und Terminierung, keine Forced-Cleanup-, Watchdog- oder Cancel-Fälle und leere Cleanupbeobachtungsfehler. Seine aktuelle Gruppe ist per Signal 0 als abwesend bestätigt.

Der geladene Encoder ist tatsächlich FP16; Checkpoint und Featurebrücke bleiben F32. Die Modell-/CPP-Decoder-SHAs stimmen mit dem eingefrorenen Profil überein. Ein tatsächlicher `mlx-cache-budget-installed`-Receipt bestätigt **268435456 Bytes /256 MiB** nach erfolgreichem Setter-Aufruf. Wie im RAM-Readback erläutert ist dies eine wiederverwendbare Cachepolicy, kein harter Gesamt-RAM-Deckel.

Der Log enthält **92 externe MLX-Encoderbelege**, **zwei vollständige native CPP-Langfallbelege** und die sechs verbleibenden Ergebnisse sind leer. Die beiden nativen Fälle sind `constructed-de-masked-300s` und `diagnostic-de-controlled-300s`, jeweils mit 4800000 vorbereiteten Samples. Die sechs Empty-Fälle sind DE-/EN-Silence, DE-/EN-Synthetic-Noise und die zwei Calibration-Room-Aufnahmen; alle haben unverändert leere Referenz und Ausgabe.

Die einzelnen Decoderereignisse enthalten keine Fall-ID. Ihre Zuordnung beruht auf dem geprüften sequentiellen Corpusloop mit genau einem beendeten Auftrag pro Fall, der identischen Manifest-/Metadatenreihenfolge und allen 94 nichtleeren Outputs. Für sechs Subsekunden-Inputs werden entsprechend der unveränderten `transcribe_using`-Vorbereitung 16000 Samples erzeugt: der bestehende Mindestinput wird mit Nullsamples aufgefüllt. Dies ist in der erneuten Zuordnungsprüfung berücksichtigt; es ist keine neue Audio- oder Parameteränderung.

Loadzeit 11681 ms und Cleanup 22 ms sind Metadaten dieses Dateibackend-Laufs. Sie werden nicht als gepacete Geschwindigkeit oder Produktlatenz ausgegeben.

## Neu gerechnete Qualitätsgruppen

Das eingefrorene Unicode-13-Orakel und der strenge Comparator wurden mit den tatsächlichen hundert Rohobjekten erneut ausgeführt. Der kanonische Satz mit 90 Fällen und das Supplemental mit 10 Fällen bleiben getrennt. Die erlaubte positive WER-Differenz bleibt exakt 0.

| Satz | Split | Sprache/Klasse | Referenzwörter | CPP-Edits | Cache256-Edits |
|---|---|---|---:|---:|---:|
| canonical90 | acceptance | de challenging | 559 | 230 | 230 |
| canonical90 | acceptance | de clean | 915 | 46 | 46 |
| canonical90 | acceptance | en challenging | 99 | 5 | 5 |
| canonical90 | acceptance | en clean | 734 | 34 | 32 |
| supplement10 | acceptance | de challenging | 463 | 28 | 28 |
| supplement10 | acceptance | de clean | 32 | 4 | 4 |
| supplement10 | acceptance | en clean | 52 | 4 | 4 |
| supplement10 | calibration | de challenging | 26 | 0 | 0 |
| supplement10 | calibration | de clean | 6 | 0 | 0 |
| supplement10 | calibration | en challenging | 21 | 2 | 2 |

Alle zehn Gruppen bestehen. Die sechs Empty-Fälle erzeugen keinen unerwünschten Text. Die neue Ausgabe hat gegenüber dem bisherigen akzeptierten FP16-Lauf keinerlei vollständige Objektänderung. Die bestehenden CPP-Fehler bleiben teilweise erhalten; das Resultat behauptet keine absolute Fehlerfreiheit.

Die beiden bekannten Paare wurden einschließlich **vollständigem Fallobjekt, Baselineobjekt und Kandidatenobjekt** neu gehasht und mit den unverändert kopierten alten Reviews verglichen:

- EN1679, `canceled`→`cancelled`: `9a2df46d3feae04745115be0c9021bc5078ed5e6a092b61e5a317afa50ccebad`.
- EN1681, Wiederherstellung von `And` vor `an experiment`: `5a93bed3f9d948b4420130a26cb547f3454212f6f274ecd6f97d56c208cdbb52`.

Die gesamten `changed-pairs.json`-Objekte und die Reviewdatei stimmen mit dem bisherigen FP16-Stand überein. Die kopierten Reviews enthalten nur diese zwei IDs. Es gibt keine allgemeine Schreibweisen-Ausnahme und keine Freigabe eines ähnlichen, aber anderen Paars.

## Ursprüngliche Exit 2-Completion bleibt erhalten

Der äußere Qualifier beendet sich laut eingefrorenem Runner mit Exit 2, solange nach der automatischen Auswertung noch ein ungeprüftes vollständiges Änderungspaar existiert. Genau diese Situation ist in den Originaldateien erhalten: alle numerischen Gates sind true, zwei Paare stehen vor dem Review auf `accepted=false` und `semanticReviewComplete=false`. Der Modellprozess selbst ist erfolgreich beendet.

Die anschließende unabhängige Prüfung verwendet ausschließlich die exakten alten Reviews und rechnet die finalen Comparatorobjekte damit neu. Diese Objekte stehen im unabhängigen JSON unter `quality.*.afterReview`; alle sind semantisch vollständig freigegeben. Die neue Acceptance bindet sowohl die originale Completion und die ursprünglichen Vergleichsdateien als auch die endgültige Reviewdatei und diesen unabhängigen Report. Es wurde kein historisches Passed-/Completed-/Review-Flag nachträglich in den Messdaten umgeschrieben.

## Quellen- und Inputbindungen

Geprüft sind **3649 aktuelle Dateizeugen**, die 3523 Source-Inventareinträge, die kleinen relevanten Probe-/Vorbereitungs-/Cache-/Comparatorquellen, Kontrollskripte, Modell-/Binary-/Kernel-Freeze-Bindungen, tatsächliche Rohdatei-/Metadatenhashes und alle Fallinputs anhand ihrer eingefrorenen Identität. Kleinere Audiodateien wurden zusätzlich neu gehasht; die großen Modelle, Binary/Kernel und langen Audiodateien wurden nicht erneut als große Blobs gehasht. Ihre vorher überprüften Hashbindungen und unveränderten Device-/Inode-/Größe-/Mtime-/Ctime-Zeugen bleiben die Grundlage.

Die unveränderte ursprüngliche CPP100-Referenz hat SHA `5753da227ac32a3c58b7a0d36d83c7262990901b321114a60aee6af174937525`. Der Kandidat hat wieder exakt SHA `35088cbfdec3aa1f8b9fb389b51ca5293c5020015eacb84f429fd04ce022b3b1`, ebenso wie der bisherige FP16-Lauf. Der Cachepolicy-Kontext ist an den vorangegangenen tatsächlichen Runtime-RAM-Lauf und den frischen Build gebunden.

Die Metadaten enthalten `sourceRevision=null`; es wird daher kein zusätzlicher Git-Revisionsbeweis behauptet. Die Identität wird hier durch die genannten eingefrorenen Source-/Build-/Inputbindungen belegt.

Dieser Nachweis erlaubt den nächsten separat autorisierten Qualitäts-/Bestätigungsschritt. Er ist keine neue prospektive Bestätigung, kein gepaceter L-/p95-Nachweis, keine vollständige Resourceprüfung und keine Produktfreigabe. Keine Modelle, Builds oder Produktionsquellen wurden von diesem Reviewer ausgeführt oder geändert.
