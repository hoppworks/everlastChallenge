# Lokaler nativer Aufnahmeablauf ohne Tastendruck, 19.10.2026

Im installierten Entwicklerbuild `f592fcd` wurde der lokale Ablauf tatsächlich
über die nativen Appknöpfe bedient: **Aufnehmen → Aufnahme läuft → Stoppen →
Kein Sprachinhalt erkannt**. Der lokale Statuskanal bestätigte Session 1 und
`Recording`; danach war die App wieder bedienbereit, ohne Text und mit
deaktivierten Kopieren-/Löschen-Aktionen. Der Agent führte Start und Stopp aus.
Keine physische Taste oder Nutzeraktion war während des Tests erforderlich.

Es wurde keine Sprachfixture abgespielt. Die Umgebungsgeräusche wurden nicht
unabhängig vermessen; der Versuch wird deshalb nicht als Nachweis absoluter
akustischer Stille oder allgemeiner Halluzinationsfreiheit ausgegeben.

Ein zweiter kurzer lokaler Start erzeugte Session 2. Ein absichtlicher
`test-control cancel 2` wurde abgelehnt, weil die Sitzung über den normalen
Appknopf und nicht vom Testzugang gestartet war. Der Status zeigte dieselbe
weiterlaufende Aufnahme. Anschließend betätigte der Agent den nativen
**Abbrechen**-Knopf; `Idle`, `cancelled` und Textlänge null wurden bestätigt.
Damit ist der Besitzschutz des Testzugangs auch an einer echten laufenden
Aufnahme belegt.

Es wurde ausschließlich das Systemstandard-Mikrofon verwendet. Keine
Audiodatei oder gesprochenen Inhalte gespeichert, keine Rechte-/Geräteänderung.
[Binär- und Rohbelegbindung](native-local-control-2026-10-19.json).

Dies belegt den autonomen lokalen Aufnahme-/Stopp-/Abbruchablauf. Es ersetzt
weder eine bestätigte externe Zielerfassung noch automatische Übergabe,
physische Hotkeyzustellung, Starting-/Finalizing-Abbruch oder vollständige
A07-/A11-/A12-Abnahme. Der externe Fokus bleibt als gesonderter Punkt offen.
