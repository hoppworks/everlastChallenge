# P04: ein vollständiger300s-Lauf mit Wartephasendiagnose

Stand: 17.10.2026. Ein Versuch nach Freeze f123a01, unverändertes Entwicklerbinary
83e8b17/9c5d1e…; keine Inferenz, kein QoS-Override, kein Mikrofon.
Ursprünglicher Launcher47139 und Prozess3639 beendet, Exit0.
Start17:17:31.894452, Ende17:22:31.973254 UTC.

Alle15000 Publikationen mit jeweils320Frames in20ms-Abständen erhalten.
Kein Chunk vor seinem Sollende veröffentlicht. Maximale Abweichung270667ns
(0,270667ms), festgeschriebene Toleranz10ms. Vollständige4,8MFrames stimmen
am Audioausgang exakt mit dem Eingang überein. Ein Start, ein Stop, ein Ende,
Join bestätigt; keine Audio-/Beobachtungs-/Joinfehler.

| Beobachtung | Maximum |
|---|---:|
| Reservierungsintervall | 12875ns |
| Schlafüberschreitung | 2091833ns |
| Uhrabstand im finalen Spin | 232666ns |
| Warteaustritt bis Publikation | 14375ns |

Stop am Audioowner300000015375ns nach bestätigtem Start,15,375µs nach dem
geplanten Ende. Ende beobachtet nach300003053542ns. Kein Controller beteiligt;
diese Werte sind keine Stop-bis-Text-Latenzen. Zusammenfassung erfasst alle
15000 Publikationen, kein ungültiger Einzelfall und somit kein Detaildatensatz.
Alle Sollzeiten/Framebereiche und tatsächlichen Intervalle maschinell geprüft.

Es trat kein Fehler auf, dessen Phase sich lokalisieren ließe. Ein einzelner
bestandener Lauf erklärt die historischen Ausreißer nicht und belegt keine
dauerhafte Schedulingstabilität. Anderer Aufbau als zentralev2: kein Modell,
QoS, Controller oder native Übergabe. Keine ASR-/L-/Ressourcenqualifikation.
Der [Bericht](replay-wait-long-v1.json) bindet Freeze, Quellpins, Binary-/Audiohash,
Host und die drei privaten0600-Rohdateien. Keine alten Ergebnisse ersetzt.
