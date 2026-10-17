# Zentraler Echtzeitvergleich v1: vollständig erhaltener Fehlversuch

Stand: 17.10.2026. Der vorab [eingefrorene Referenzlauf](paced-reference-protocol.md)
ist beendet: Start 14:39:57.579991 UTC, Ende 14:57:53.854355 UTC, Exit 1.
PID 79998 und der zugehörige Launcher sind terminal; der Prozess wurde nicht
neu gestartet. Werkzeugrevision `b62f86f`, Binary- und Parameterbindungen
bleiben die vorab dokumentierten Werte.

## Tatsächlicher Abbruchgrund

Alle 181 vorgesehenen Slots sind im Bericht vorhanden:

- 102 `ok`, darunter der eine Warm-up und 101 gewertete Läufe;
- ein `invalid_setup` beim englischen 5-s-Fall, Wiederholung 12;
- 78 ausdrücklich `unavailable`, nach dem Fehler nicht ausgeführt.

Im fehlerhaften Slot `run-fleurs-en-test-1679-12` kam ein Chunk zu spät:
Frames 2240..2560, geplanter Veröffentlichungszeitpunkt 160.000.000 ns ab t0,
beobachteter Zeitpunkt nach Veröffentlichung 171.606.084 ns. Die Abweichung
von **11.606.084 ns** überschreitet das vorab festgelegte 10-ms-Budget um
1.606.084 ns. Von den 250 Publikationen dieses Slots ist genau eine verspätet.
Die Veröffentlichung selbst dauerte 2542 ns; die Verzögerung lag bereits
vor deren Beginn. Die genaue Ursache der Threadverzögerung wurde nicht
instrumentiert und wird nicht als bewiesener Betriebssystemfehler ausgegeben.

Der Stopzeitpunkt lag bei 5.000.016.209 ns, also 16.209 ns nach dem geplanten
Ende und innerhalb seines Budgets. Capture-/Controlleruhr stimmten überein;
der Controller erreichte lokalen finalen Text. Diese Beobachtungen machen die
verspätete Eingabe nicht nachträglich gültig. Die rohe Stop-bis-Text-Differenz
dieser ungültigen Zeile bleibt im Zahlenbericht erhalten, zählt aber nicht als
bestandene Echtzeitmessung.

Cleanup wurde ohne Fehler bestätigt und alle Besitzer gejoint, beobachtet nach
6470 µs. Servicebereitschaft: 1287 ms. Das sind Einzelbeobachtungen dieses Laufs,
keine zusätzliche Kaltstart- oder Ressourcenqualifikation.

## Folgen für die Abnahme

Die Statistik ist vollständig unterdrückt: **`L: null`**, keine Fallmediane oder
p95-Werte aus einem erfolgreichen Teilbestand, `measurementComplete: false`,
`qualified: false`. Es werden keine fehlenden Zeilen entfernt, keine einzelnen
Läufe wiederholt und keine Jittergrenze nach Sichtung des Fehlers erhöht.
Auch die zuvor fertig durchlaufenen deutschen Fälle werden nicht allein als
bestandener Sechs-Fälle-Vergleich ausgegeben.

Der [inhaltfreie vollständige Zahlenbericht](paced-six-reference-v1.json)
enthält alle Slots mit Rohzeiten/Status, den ungültigen Chunk und Hash-/Modus-
bindungen der vier lokalen Rohdateien. Hypothesen bleiben unter
`artifacts/paced-six/v1/` privat; die numerische Dokumentation druckt sie nicht.
Der Completionbeleg bindet Exitstatus, ursprünglichen Aufruf, Binary und
Rohberichte. Der Fehlversuch ist Evidenz für einen ungültigen Messaufbau in
dieser Zeile, kein automatischer Nachweis eines Decoder-/Audioverlustfehlers.

Ein späterer weiterer Vergleich benötigt vorab festgelegte Bedingungen und
bleibt getrennt von diesem unveränderten Fehlversuch. Ohne vollständigen
gültigen Vergleich sind die zentralen L-/Median-/p95-Abnahmen weiter offen.
