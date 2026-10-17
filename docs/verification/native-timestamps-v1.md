# P05: native Zeitmarken verbessern Zielbefunde, Restfehler bleiben

Stand: 17.10.2026. Prospektiv eingefrorener Diagnosevergleich, kein neuer Blindtest.
Freeze6d9f7ff, Code/Binary0a94a09. Gleiche drei Eingaben und gleiches neu gebautes
Binary in zwei seriellen Prozessen. Ausschließlich no_timestamps variiert;
alle übrigen Decoder-/VAD-/Energieparameter unverändert, Produkt behält Baseline.

Launcher30028 terminal; Baseline PID99634 16:51:45.804668–16:52:02.125197 UTC,
Kandidat PID99677 anschließend bis16:52:21.071210 UTC. Beide Inferenz- und
Auswerteprozesse Exit0; sechs Fälle statusok. Sidecar bestätigt noTimestamps
true beziehungsweisefalse. Die drei neuen Baseline-Ergebniszeilen sind exakt
identisch zu den gebundenen historischen Beobachtungen.

| Fall | Baseline Edits/Refwörter | Native Zeitmarken Edits/Refwörter |
|---|---:|---:|
| Original DE1662 | 0/21 | 0/21 |
| Schwache isolierte Maske | 21/21 | 1/21 |
| Kontrollierte 300 Sekunden | 147/463 | 28/463 |

Die kurze komplette Absatzdopplung entfällt, ein zusätzliches Anfangswort bleibt.
Im Langfall sind alle21 Quellblöcke wieder erkennbar statt vier vollständigen
und einem teilweisen Verlust;35 akzeptierte Segmente statt10. Dies ist keine
fehlerfreie Inhaltsabnahme: Die Kochanweisung DE1674 ist trotz korrekter Zeitangaben
verfälscht; DE1675 zitiert Wörter falsch. Weitere Namen-/Wortfehler sowie eine
zusätzliche terminale Musikmarkierung bleiben. Keine verlorene explizite Negation
oder eindeutig falsche Zahl im geprüften Kandidaten gefunden; lokale Fehler
werden dadurch nicht als behoben ausgegeben.

Alle sechs kompletten rohen Ergebnispaare und alle21 Quellblöcke wurden manuell
semantisch geprüft. Die Bedeutungsprüfung ist kein WER- oder Stringgleichheits-
Automatismus. Der [inhaltfreie Bericht](native-timestamps-v1.json) bindet private
Rohdaten, vollständige private Bedeutungsprüfung und alle Zahlen. Die Ausgaben
werden weder entdoppelt noch gefiltert oder für eine Referenzreparatur benutzt.

Der einzelne Flagwechsel beeinflusst diese Zielbefunde deutlich. Er beweist
weder exakte Seek-/EOT-/Fallbackentscheidungen noch eine akustisch geprüfte
Ausrichtung. Vor einer Produktänderung bleiben vollständige Kalibrierung und
relevante Abnahme erforderlich. Keine P05-/P06-/L-/Ressourcen-/Abbruchqualifikation;
kein App-/DMG-Build und kein Mikrofonzugriff.

Unabhängiger vollständiger Readback reproduzierte alle sechs Auswertungen und
prüfte sämtliche Satz-/Blockurteile. Eine Baseline-Notiz zu DE1671 war zu weit:
Der Finanzierungsbeginn ist schon vorhanden, erst die nachfolgende FBI-Zehn-
Agenten-Klausel fehlt. Neue private Reviewversion v2 präzisiert nur diese Notiz;
Original v1, Rohtexte, Urteile und Zählungen bleiben unverändert erhalten.
