# Echte Mikrofonaufnahme und einmalige Übergabe ins eigene native Ziel

Am 19.10.2026 lief der lokale Entwicklertest aus `212d5e7` erfolgreich durch:
Ein einmaliger direkter Nutzerklick stellte den tatsächlichen Fokus her. Danach
übernahm der Agent Start, Wiedergabe der festen öffentlichen FLEURS-Aufnahme
1517 über `afplay` und Stopp. Systemstandard-Mikrofon, echte CPAL-/DSP-/Whisper-
Pipeline und produktiver nativer Übergabepfad; kein Text-Injektionsadapter.
Keine Änderung von Rechten, Geräteeinstellung, Anzeige oder Modell.

Die App erkannte den festen Satz vollständig. CUA las ihn anschließend exakt
einmal im mehrzeiligen Feld des getrennten eigenen AppKit-Empfängers. Dessen
private Metadaten bestätigten expectedSentenceMatches=true, occurrences=1 und
shortcutCharacterPresent=false. Der Actor endete Idle/outcome=sent; die App
zeigte „Einfügebefehl gesendet“. Das ist ein echter Empfangsnachweis in diesem
Testempfänger, zusätzlich zur vorsichtigen Produktmeldung.

## Erstes Testergebnis und Synchronisierung

Der ursprüngliche Watcher las beim ersten Actor-Idle noch einen Empfängertick
von rund 149 ms davor. Sein konservierter Ausgang meldet deshalb false.
Diese Rohdatei bleibt unverändert. Die spätere unabhängige Veröffentlichung
und der direkte CUA-Feldinhalt bestätigen die tatsächliche Einfügung. Es war
keine zweite Aufnahme oder erneute Einfügung nötig.

Der wiederverwendbare `recipient-observation.mjs` wartet nun auf eine neuere,
zeitlich nach dem Actor-Abschluss liegende Beobachtung desselben Empfängers.
Die Wartezeit ist begrenzt; alter Erfolg, falscher Text, Duplikate, fremde PID,
Zusatzzeichen und Lesefehler bestätigen nichts. Fünf fokussierte RED→GREEN-Tests
und alle 47 Node-Tests bestanden; gezieltes Oxlint und Verträge grün.

Der persistierte CLI-Aufruf lautet:

```sh
node scripts/recipient-delivery.mjs ABS_RECIPIENT_SNAPSHOT ABS_ORIGINAL_ATTEMPT
```

Die konkreten Argumente, Sequenz-/Zeitgrenzen und Codebindungen stehen im
[strukturierten Nachweis](native-delivery-success-2026-10-19.json). Der originale
Watcher wurde nicht rückwirkend geändert; der CLI ist ein separater diagnostischer
Readback. Der damalige periodisch überschriebene Snapshot ist nicht mehr als
exakte Bytekopie seines CLI-Hashes verfügbar. Das konservierte CLI-Ergebnis
und die getrennte bestätigte Metadatenkopie bleiben erhalten; keine Rekonstruktion
fehlender Rohbytes und kein zusätzlicher Aufnahmeversuch.

## Grenze und Abschluss

Start/Stopp erfolgten über den expliziten begrenzten Entwicklerendpunkt, nicht
über den physischen globalen Hotkey. Dieser Lauf ist auch kein Nachweis eines
Browser-/Quarantäne-/Frischdaten-/Offline-Starts des normalen Releasebundles.
Weitere Zielprogramme, Negativfelder und vollständige OS-Abnahme bleiben offen.
Der unabhängige Readback bestätigt Binärbindungen, Empfängeridentität, Zeitfolge,
Belegumfang und Synchronisationsdelta. Keine vollständige P02/P07/P10-Abnahme.
Beide Testprogramme wurden regulär beendet; die normale App wieder geöffnet.
