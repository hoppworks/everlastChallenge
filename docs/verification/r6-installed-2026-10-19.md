# R6: lokale Installation und Rechtewiederherstellung am 19.10.2026

Die exakte R6-Roundtrip-App ist seit 11:38:29 UTC unter
`/Applications/Everlast Dictation.app` installiert. Produktquelle:
`2abf49503fe8b8c43fd7916cc5be3cb9aea3be6f`.
Executable-SHA256:
`2cd8582c5d1c64c23a5d8c4cd66553aa8b132d11d4a7c8ff19346dc231b30dbe`.
Der installierte Hash wurde nach der Installation erneut direkt gelesen und
stimmt mit dem [R6-Paketbeleg](testversion-package-r6.md) überein.

Die vorherige R4 wurde regulär beendet; PID83623 war vor dem Austausch
abwesend. Originalbundle und zusätzliche Rückfallkopie bleiben unter
`artifacts/r6-installed-acceptance-v1/r4-installed-original.app` beziehungsweise
`r4-backup.app` erhalten. Einstellungen und Modelle wurden weiterverwendet.
Dies ist keine Frischdaten-, Browserdownload- oder Quarantäneabnahme.

Die laufende App zeigte zunächst weiter den vorherigen Zustand. Reguläres
Beenden über das Appmenü, bestätigtes Ende der PID84729 und erneutes Öffnen
der exakten Applications-App behoben diese Diskrepanz: Beide Rechte werden
jetzt ausdrücklich als „Freigegeben“ angezeigt; die Hauptansicht meldet
„Bereit zum Diktieren“ mit „Standardmikrofon · Deutsch“.

Die Ursache des verzögerten Status ist damit nicht abschließend isoliert.
Keine erneute Signierung oder Kompilierung, keine fremde Zugriffsvergabe und
keine Änderung der Anzeige- oder Mikrofoneinstellungen. Ein neuer positiver
Hotkey-/Übergabenachweis in R6 steht noch aus; keine vollständige A25- oder
Gesamtpaketabnahme aus Installation und Rechteanzeige.

Lokale Rohbelege: `artifacts/r6-installed-acceptance-v1/preflight.json`,
`installation.json`, `permission-recovery.json`; UI-Zustände direkt mit CUA
beobachtet. Vorinstallation: Standardeingang70 inaktiv, Standardausgang75
mit eingebautem Transport. Kein weiterer Gerätetest daraus.
