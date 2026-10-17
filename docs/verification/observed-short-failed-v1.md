# P05: erste native Beobachtung technisch fehlgeschlagen

Stand: 17.10.2026. Freeze3dbd60f, Implementierung/Binary0959b2f.
UrsprünglicherLauncher62240/PID96789 terminal, InferenzExit1,
16:37:17.735388–16:37:21.728907 UTC; AuswerterExit2.

Beide Fälle wurden mit explizitem Beobachtungsfehler exportiert. Die damalige
allgemeine Fehlermeldung unterscheidet ungültige Metadaten und überschrittene
Segment-/Textgrenzen nicht. Tatsächliche Werte und Anzahl wurden nicht erhalten.
Daher sind weder Ursache noch native Segmente aus diesem Lauf rekonstruierbar;
kein erfolgreicher Leertext oder gültiger Textvergleich wird behauptet.

Zusätzlich lehnte der bestehende WER-Auswerter die angereicherten Diagnosezeilen
ab: Er akzeptiert ausschließlich id/status/text. Keine gültige Qualitätsdatei
entstand. Der Auswerter und seine strenge Vertragsprüfung werden nicht gelockert.

Beide Defizite werden in der Diagnose korrigiert: tatsächliche ungültige numerische
Metadaten in typisiertem Fehler erhalten; primäre Ergebnisdatei im bestehenden
Dreifeldschema, volle Beobachtungen und Fehler in separater Metadatendatei.
Keine Änderung von Decoderparametern, Text oder Eingaben. Ein neuer Versuch
benötigt neue Quellen-/Binarybindung und neue Ausgabepfade. Der Fehlversuch bleibt
vollständig erhalten und wird nicht nachträglich als bestandene Messung ersetzt.

[Inhaltfreier Fehler-/Hashbericht](observed-short-failed-v1.json) bindet alle
privaten 0600-Dateien. Keine P05-/P06-/L- oder Ursachenqualifikation.
