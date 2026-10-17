# P05: native Kurzbeobachtung v2 – negative Rohzeit sichtbar

Stand: 17.10.2026. Implementierung6042cad, Freeze b95cfd2, eigene neue Ausgaben.
Launcher42782/PID97521 terminal: Inferenz Exit1,
16:40:37.752929–16:40:41.767653 UTC; Auswerter Exit0. Beide primären Fälle
korrekt als failed ohne Text/WER ausgewiesen, Exportvertrag jetzt kompatibel.

Die typisierten Fehler erhalten erstmals tatsächliche numerische Metadaten:
bei beiden Fällen Segmentindex0, Start−100730, Ende3000 Zentisekunden;
No-Speech-Wahrscheinlichkeit jeweils endlich und im Einheitsintervall.
Die negative Startzeit ist der belegte Ablehnungsgrund dieses Versuchs.
Die Segmenttexte wurden dadurch noch nicht exportiert; keine Aussage über
Dopplungsort oder tatsächliche native Fensterverwerfung daraus möglich.

Bei unverändertem no_timestamps=true sind native Segmentzeiten keine garantierte
Ausrichtung. Die derzeitige Beobachtung setzt nichtnegative Zeiten voraus und
bricht daher die eigentliche Textdiagnose ab. Im Vendor wird der Segmentbeginn
bei der Ausgabe aus Seek und dem Timestamp-Kandidaten des ersten Tokens
berechnet (whisper.cpp, Segmentausgabe). Die beobachtete Rohzeit wird nicht
repariert oder als echte Audiozeit ausgegeben. Die folgende reine Messkorrektur
soll signed Rohzeiten erhalten und deren Nichtnegativität/Reihenfolge separat
kennzeichnen. Index-, Wahrscheinlichkeits- und Speichergrenzen bleiben geprüft.

[Inhaltfreier Fehler-/Hashbericht](observed-short-failed-v2.json) bindet die
privaten Rohdateien und gültige Auswertung der zwei fehlgeschlagenen Fälle.
Decoderparameter, Audiodaten und frühere Fehlversuche bleiben unverändert;
keine P05-/P06-/L- oder konkrete Decoderursachenqualifikation.
