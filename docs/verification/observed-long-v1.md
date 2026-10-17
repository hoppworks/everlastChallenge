# P05: vollständiger ASR-Eingang, unvollständiger nativer Langtext

Stand: 17.10.2026. Unverändertes kontrolliertes 300-s-Signal, erste akzeptierte
native Langbeobachtung. Implementierungad83f3f, Freeze0938f48; keine Parameteränderung.

Launcher48275/PID98677 terminal, Inferenz Exit0,
16:46:07.082484–16:46:20.381979 UTC; Auswerter Exit0. Ergebniszeile byteidentisch
zum vorherigen kontrollierten Langlauf:147/463Edits,335Hypothesentokens.
Tatsächlicher Eingang, ausgewählter Bereich und ASR-Input jeweils4.800.000Frames,
Range0..4800000, kein Padding. Der vollständige Signalbereich wird verarbeitet.

Zehn akzeptierte native Textsegmente. Direkte Verkettung und abschließendes
Trimmen entsprechen exakt dem Endtext. Die bereits dokumentierten fehlenden
Inhalte stehen auch in diesen Segmenttexten nicht. Damit liegt der sichtbare
Inhaltsverlust vor der Rust-Textverkettung. Es gibt keine Grundlage für eine
nachträgliche Textreparatur oder für eine bestandene Vollständigkeitsabnahme.

Die nativen Rohendzeiten lauten3000,6000,…,30000Zentisekunden; Rohstartzeiten
sind negativ. Das numerische Zeitflag ist jeweilsfalse, keine Audioausrichtung
behauptet. Alle exportierten NSP-Werte sind endlich im Einheitsintervall.
Zehn akzeptierte Segmente beweisen keine Zahl sämtlicher Encoder-Versuche oder
verworfener Fenster. Seek/EOT, dynamische Historie und Fallbacktemperatur bleiben
unbeobachtet. Quelle und Muster begründen einen gezielten Zeitmarkenvergleich,
keine bereits bewiesene innere Decoderursache.

[Inhaltfreier Bericht](observed-long-v1.json) bindet sechs private0600-Dateien,
Frame-/Zeitwerte und Segmenttexthashes. Das ursprüngliche stark verrauschte
Abnahmesignal, Kontrollsignal und alle bisherigen Rohdaten bleiben unverändert.
Keine P05-/P06-/L-/Ressourcen-/Abbruchqualifikation.
