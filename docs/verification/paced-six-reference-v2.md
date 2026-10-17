# Zentraler Echtzeitvergleich v2: Fehlversuch mit Publisher-QoS

Stand: 17.10.2026. Der getrennte, vorab eingefrorene v2-Lauf ist beendet:
Start **15:33:41.096036 UTC**, Ende **15:48:00.061931 UTC**, **Exit 1**.
Root bestätigte PID 88303 und Launcher 30830 als terminal. Dieser Dokuauftrag
hat keine Prozesse gesteuert und keinen Lauf neu gestartet.

Die Werkzeugrevision ist `fbbab45`. Der
[vorab eingefrorene Laufplan](../../tests/fixtures/corpus/paced-six-reference-run-v2.json)
bindet Binary, Parameter, Modell und dieselben sechs Referenzfälle. Die
[Parameterdatei](../../tests/fixtures/corpus/paced-six-reference-parameters-v2.json)
hat SHA-256 `ec18cb5d8291c3481cde54ac77a45b8c7bb584807de427301d4875e5739f0667`.
Sie behält 20-ms-Chunks, 10-ms-Jitterbudget und die 250-µs-Endstrecke bei.
Neu erprobt wird ein begrenztes `USER_INTERACTIVE/0`-QoS-Override ausschließlich
für den Publisher während der vorgesehenen Feed-/Stop-Phase. Dies ist keine
Echtzeitgarantie; weder Basis-QoS noch System-/Energiespareinstellungen wurden
für diesen Dokuauftrag geändert. Die vollständige Quellbindung bleibt maßgeblich;
der Vergleich allein isoliert keine Betriebssystemursache.

## Tatsächlicher Abbruch

Alle **181 Slots** bleiben erhalten:

- **81 `ok`**: ein Warm-up und 80 gewertete Wiederholungen;
- **ein `invalid_setup`**: `run-fleurs-de-test-1668-21`, deutscher 15-s-Fall,
  Wiederholung 21, Session 82;
- **99 `unavailable`**: nach diesem Fehler ausdrücklich nicht ausgeführt.

Von insgesamt **38.500 Publikationen** liegen 38.498 im Budget. Genau zwei
Publikationen des fehlerhaften Slots sind verspätet:

| Frames | Geplant ab t0 (ns) | Vor Publish (ns) | Nach Publish (ns) | Verspätung nach Publish (ns) | Publishdauer (ns) |
|---|---:|---:|---:|---:|---:|
| 88640..88960 | 5.560.000.000 | 5.572.043.208 | 5.572.045.041 | 12.045.041 | 1833 |
| 100160..100480 | 6.280.000.000 | 6.292.328.916 | 6.292.330.500 | 12.330.500 | 1584 |

Beide Überschreitungen bestanden bereits **vor** dem Publikationsaufruf.
Die 10-ms-Grenze wird nach Publish um 2.045.041 beziehungsweise 2.330.500 ns
überschritten. Die kurze Publishdauer macht die Eingabe nicht gültig; die
konkrete Ursache der vorherigen Threadverzögerung wurde nicht instrumentiert.
Insbesondere ist damit weder ein Betriebssystemfehler noch ein Decoder- oder
Audioverlustfehler bewiesen.

Der Stopzeitpunkt lag bei **15.000.003.083 ns** ab t0, also 3083 ns nach dem
geplanten Ende. Controller- und Captureuhr stimmten überein; der Controller
beendete den Slot mit lokalem finalem Text bei **16.095.327.458 ns**.
Die rohe Differenz beträgt **1.095.324.375 ns**. Sie bleibt ausschließlich als
Beobachtung der **ungültigen** Zeile erhalten: **kein gültiger Latenzwert, kein L**.
Der rechtzeitige Stop und das lokale Ergebnis heilen die verspätete Eingabe nicht.

QoS-Eintritt und -Abschluss wurden bei allen 82 tatsächlich begonnenen Slots
bestätigt, einschließlich des fehlerhaften Slots; kein QoS-Fehler wurde gemeldet.
Bei den 99 nicht ausgeführten Slots bedeuten die beiden `false`-Werte
„nicht ausgeführt“, nicht einen fehlgeschlagenen QoS-Aufruf. Erfolgreiches
Override bedeutet hier ausdrücklich keine eingehaltene Jittergrenze.

Cleanup bestätigte fehlerfreies Join aller Besitzer nach **5456 µs**;
Servicebereitschaft wurde nach **1330 ms** beobachtet. Das sind Einzelwerte,
keine zusätzliche Kaltstart-, Ressourcen- oder Hardwarequalifikation.

## Unveränderte Abnahmegrenzen

**`L: null`, `statistics: null`, `measurementComplete: false`, `qualified: false`.**
Es gibt keine Fallmediane, p95-Werte oder Gesamtstatistik aus dem erfolgreichen
Teilbestand. Die 80 erfolgreichen Wiederholungen ersetzen nicht die fehlenden
100 gültigen Wiederholungen. Kein Slot wird entfernt oder selektiv wiederholt;
Jitterbudget, Referenzen und Parameter werden nicht nachträglich passend gemacht.

Der [vollständige inhaltfreie Zahlenbericht](paced-six-reference-v2.json)
enthält alle 181 Zeilen, deren Feedzusammenfassungen und QoS-Ergebnisse, beide
ungültigen Publikationen sowie SHA-256, Größe und Modus der vier unveränderten
Rohdateien. Alle vier Dateien unter `artifacts/paced-six/v2/` haben Modus 0600.
Die Hypothesen werden nicht in diese öffentliche Dokumentation übernommen.
Completionbeleg, Laufplan und Parameterbindungen wurden gegen die tatsächlichen
Dateien geprüft; Modell und Binary wurden durch diese Archivierung nicht geladen.

[v1](paced-six-reference-v1.md) und v2 bleiben getrennte, unveränderte Fehlversuche.
Auch v2 begründet keine gültige Sechs-Fälle-L-Messung, Modellwahl oder vollständige
P05-Abnahme. Eine spätere Ursachenuntersuchung oder ein weiterer Lauf muss eigene
Bedingungen und Belege haben; daraus folgt kein rückwirkend gültiges Ergebnis.
