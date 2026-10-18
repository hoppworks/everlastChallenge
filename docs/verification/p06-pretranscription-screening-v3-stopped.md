# P06 – dritter Versuch für kontrollierte Übernahme beendet

Am 18.10.2026 wurde der laufende v3-Versuch nach dem Fortsetzungswunsch
kontrolliert per SIGINT an den zuvor identifizierten eigenen Launcher beendet.
Die Messsession endete mit Exit 1: 99 Fälle ok, der gestartete Fall 100
`diagnostic-de-controlled-300s` fehlgeschlagen, 300 Fälle unavailable.
Alle 400 Abschlüsse und 800 logischen Zeilen bleiben erhalten.

Der unterbrochene Wrapper 64348 endete mit -9. Der Launcher konnte das
Prozessgruppenende innerhalb seiner Frist nicht bestätigen;
`terminationConfirmed=false` bleibt unverändert erhalten. Ein späterer
unabhängiger Betriebssystemcheck um 13:37:58 UTC fand alle 201 bekannten
PIDs und 100 eigenen Prozessgruppen abwesend. Dieser private, an Launch und
Gesamtabschluss gebundene Nachweis schließt nur die spätere Startbarriere.
Die eigene Idle-Sleep-Anforderung ist beendet. Keine fremden Prozesse wurden
beendet und keine globale Energieeinstellung geändert.

Die [inhaltfreie Projektion](p06-pretranscription-screening-v3-stopped.json)
bindet Freeze, Start, Gesamtabschluss, alle 400 Einzelabschlüsse und alle
vorhandenen Rohdateien. Hashes, Größen, private Dateimodi und Übereinstimmung
von Einzel- und Gesamtabschluss sind geprüft. Es ist keine erneute vollständige
Timing-, Text- oder Leistungsabnahme. Sämtliche privaten Dateien bleiben
unverändert; kein v3-Fall wird in das neue Screening übernommen.

Die geplante [kontrollierte Übernahme](p06-pretranscription-continuation-v1.md)
verwendet ausschließlich die 185 festgelegten gültigen ganzen v2-Fälle und
215 neue Prozesse. V3 bleibt ein beendeter unvollständiger Versuch.
Keine Qualitäts-, Latenz-, Ressourcen-, Aktivierungs- oder P06-Fertigmeldung.
