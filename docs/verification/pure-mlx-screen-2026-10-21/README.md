# Zusätzlicher reiner MLX-ASR-Lauf, 21.10.2026

Sechs öffentliche Fälle, je ein Prozess pro Variante, mit geladenem Modell für
alle sechs aufeinanderfolgenden Jobs. Derselbe eingefrorene Probe-Build aus
2ded384; spätere UI/Native-Delivery-Änderungen verändern diese ASR-Implementierung
nicht. Modelle unterscheiden sich: Hybrid nutzt den qualifizierten Q5-abgeleiteten
F16-Encoder, der MLX-Kandidat den gepinnten vollständigen FP16-Checkpoint.

| Beobachtung | Hybrid | MLX-ASR |
|---|---:|---:|
| Ganze Prozesslaufzeit inklusive Laden und sechs Jobs | 4,05 s | 2,96 s |
| Maximaler Resident-Speicher laut macOS `time -l` | 1,34 GB | 1,74 GB |
| Fehler auf fünf deutschen Fällen / 119 Referenzwörter | 11 | 10 |
| Fehler auf einem englischen Fall / 34 Referenzwörter | 0 | 2 |

Vier deutsche Texte sind wortgleich, beim fünften verändert sich die Ausgabe.
Der englische Kandidat schreibt „the certification“ statt „decertification“:
der bereits bekannte Bedeutungsfehler ist weiterhin vorhanden. Es gibt keinen
belegten RAM-Gewinn in diesem Lauf. Die einzelnen Laufzeiten sind keine Mediane,
kein kontrollierter Speedup-Nachweis und keine App-Latenz nach Aufnahmeende.
Feste Reihenfolge (Hybrid zuerst) und mögliche Betriebssystem-Caches bleiben
Einschränkungen. Nach diesem Qualitätsbefund keine zusätzliche Timingkampagne.
