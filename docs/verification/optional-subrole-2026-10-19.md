# Optionale macOS-Unterrolle, 19.10.2026

Der bisherige Zielreader wandelte sämtliche Fehler beim Lesen von `AXSubrole`
in unbekannte Sicherheitseigenschaften um. Das lehnte auch einen bestätigten
Textbereich ab, dessen optionale Unterrolle regulär fehlt.

Der Reader unterscheidet jetzt vorhandene Zeichenfolgen, dokumentierte
Abwesenheit, echte Abfragefehler, falsche Werttypen und leere Erfolgsantworten.
Ausschließlich `AXAttributeUnsupported` (−25205) und `AXNoValue` (−25212) werden
bei `AXTextArea` als fehlende optionale Unterrolle behandelt. Alle anderen
Sicherheits-, Editierbarkeits-, Fokus-, Prozess- und Sitzungskriterien bleiben
erforderlich. Andere Rollen ohne Unterrolle bleiben gesperrt.

Der eigene unsichtbare AppKit-Test bestätigt: `NSTextView` meldet `AXTextArea`
mit leerer Unterrolle; die normale Textfeldzelle meldet ebenfalls keine
Unterrolle, die Passwortfeldzelle dagegen `AXSecureTextField`. Das unsichtbare
Textview meldet sich deaktiviert. Der Test behauptet daher weder ein tatsächlich
einfügbares Feld noch einen bestimmten AX-Transportstatus oder einen Notes-Test.

Ein gezielter Reader-Test schlug vor der Korrektur an der positiven
Abwesenheitsbedingung fehl und besteht danach samt negativen Gegenfällen.
Vier Zieltests und striktes Clippy sind grün. Der autonome Verbundlauf
`artifacts/autonomous-tests/1792415606087-71288c2c-b9ba-4e04-80d7-d03e2ce25719/report.json`
bestand alle neun Schritte mit 85 Rust- und 15 Node-Tests. Unabhängiger Review
der Änderung: keine verbleibenden Befunde.

Diese Korrektur beweist noch nicht die Ursache des zuvor zurückgehaltenen
Hotkeyergebnisses. Ein echter positiver Übergabenachweis bleibt erforderlich.
Keine neue ASR-Qualifikation, Mikrofonaufnahme oder formale Paketfreigabe.
