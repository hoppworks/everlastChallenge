# Native Tastaturführung im Abschlusskandidaten

Der normale Build aus `b799936` (Binary `709da0a462109526edbffbaa190f41718add6b8a657dd4b17a5a7f957d282e63`) zeigt beim Öffnen den tatsächlichen Tab-Fokus. Pfeiltasten verschieben den sichtbaren blauen Fokusring in Lesereihenfolge; Leertaste aktiviert den fokussierten Tab. Modell → Berechtigungen → Aufnahme funktioniert auch rückwärts. ⌘W schließt, ⌘, öffnet mit erhaltenem Tab-Fokus wieder. Mikrofon und Accessibility blieben freigegeben; kein weiterer Rechtezyklus. Danach Hauptfenster bereit, Ergebnis leer, Systemstandard/Deutsch/⌥D unverändert.

Der Fehler war mit der unveränderten Regular-App-Probe unter aktivem Vordergrund-/Key-Fenster reproduziert. Ein passiver eigener Ereigniszähler bestätigte die Tastenzustellung. Der öffentliche Lifecycle-Vertrag schlug vor der Korrektur fehl und besteht mit initialem Fokus, drei Wiederöffnungen und erhaltenem bestehendem Texteingabefokus. Die Probe selbst repariert keinen Responder. Der Produktfix setzt den konfigurierten Einstieg ausschließlich bei fehlendem oder eigenem Fenster-Responder.

FullKeyboardAccess blieb ausgeschaltet. Die reguläre Tab-Taste nimmt deshalb nicht sämtliche Buttons/Popups in die Systemnavigation auf; das ist keine behauptete vollständige Tab-Runde. Ctrl-Tab schaltete in der Probe nicht um. Die native Pfeil-/Leertastenbedienung ist tatsächlich beobachtet; Start/Stopp/Abbruch/Kopieren sind durch die bestehenden nativen Tastaturbefehlsverträge abgedeckt. Keine Aufnahme, kein globaler Hotkey und kein OS-Einstellungswechsel wurden für diese Prüfung ausgeführt.

[Strukturierte Belege mit Hashbindungen](native-preferences-keyboard-2026-10-19.json). Der bestandene G-Lauf bleibt am ursprünglichen R11 gebunden; für den reinen UI-Änderungsumfang gilt der getrennte Übertragbarkeitsabgleich.

![Tatsächlicher blauer Fokusring im installierten normalen Build](../../artifacts/native-preferences-final-normal-v1/focus-model.png)
