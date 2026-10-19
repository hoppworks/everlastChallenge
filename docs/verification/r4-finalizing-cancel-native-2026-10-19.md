# Tatsächlicher nativer Abbruch während der Verarbeitung

Am 19.10.2026 im unveränderten installierten R4 ausgeführt, Produktquelle
`ee1a6a4f78fca3b45026a8de835e997c914b43ec`, Binär-SHA256
`efea6ec2df880186788ff775b9beea61be8c53cd61c9485c932ca6f043bc83f5`.
Produktprozess 68490 vorhanden. [Strukturierter Beleg](r4-finalizing-cancel-native-2026-10-19.json),
lokale vollständige AX-Zustände unter
`artifacts/r4-finalizing-cancel-native-v1/proof.json`.

## Beobachtete Kette

1. Die bereits gehashte öffentliche Fleurs-Fixture in QuickTime abgespielt und
   Aufnahme über den echten Fensterbutton gestartet.
2. Nach dem sprachhaltigen Abschnitt „Stoppen“ geklickt; tatsächlichen Zustand
   „Diktat wird verarbeitet“ samt aktiviertem nativen „Abbrechen“ gelesen.
3. Unmittelbar danach diesen Button betätigt. Angezeigtes Ergebnis:
   „Diktat abgebrochen“, Text leer und Kopieren deaktiviert.
4. Neue echte Aufnahme über „Aufnehmen“ gestartet, Zustand „Aufnahme läuft“
   gelesen und über den nativen Abbrechen-Button beendet.
5. Spätere Beobachtung unverändert abgebrochen, Text leer, Kopieren deaktiviert.
   Kein verspäteter alter Text im neuen Diktat beobachtet.

Dieser Test ergänzt den realen Finalizing-Abbruchteil von A11 und den gemischten
Inferenzabbruch→Neustart→Aufnahmeabbruch-Teil von A12. Der Aufnahmestart des
belegten Versuchs wurde um 10:17:12,422 UTC beobachtet. Das ist kein Stopp-bis-Text-
Benchmark; kein Decoder-/Paste-Hook wurde künstlich verzögert. Keine privaten
Aufnahmen oder Transkripte gespeichert, keine Rechte oder Binärdatei verändert.

## Vorherige Versuche bleiben getrennt

Der jetzt positive Nachweis benutzt den nativen Button, kein synthetisches
Escape. Der physische Escapeweg wird nicht als bestanden ausgegeben. Die
gezielte Quellprüfung zeigt unveränderliche Sitzungs-IDs und Widerruf alter
Escape-Bindungen; elf bestehende Hotkeytests bestehen. Das ist kein Repro der
ungeklärten früheren UI-Beobachtung und keine Begründung für einen Produktpatch.

Automatische Lieferung, Starting-Abbruch, Sleep/Wake und vollständige
P03-/P04-Abnahme sind durch diesen begrenzten Fall nicht geschlossen.
R5s vollständige installierte Abnahme bleibt eigenständig offen.

Ein unabhängiger Readback der abgeschlossenen AX-Belege bestätigt diese
Beobachtungskette und ihre Grenzen ohne Befund.
