# Einmaliger Start: implementierte Komponenten

Stand 19.10.2026. [Gebundene Metadaten](single-press-components-2026-10-19.json).

Der Runner prüft Produkt und eigenen Empfänger, verbraucht das feste
Gestenbudget vor dem ersten Signal dauerhaft und akzeptiert nur beobachteten
Start, Vorwarnung, Maximalstopp und byte-/hashgleichen Empfang. Verspäteter
Empfängerreadback erhält fünf Sekunden; Transport belegt keine ASR-Qualität.
15 Node-Verträge bestanden. Reviewbefunde zu fehlgeschlagener Bereinigung und
ablaufenden Exportern wurden mit RED/GREEN korrigiert: unbestätigtes Cleanup
lässt den unabhängigen Watchdog aktiv, und vor dem Impuls bleiben mindestens
600 Sekunden Beobachtungsreserve.

Der Wiedergabeadapter bindet die tatsächliche öffentliche 300-s-Datei an
SHA256, RIFF-Grenzen und 4.800.000 PCM16-Mono-Samples bei16kHz. Start kehrt
nach Prozessstart zurück, frühes Ende und Fehler verhindern Erfolg. Zehn
Tests bestanden, einschließlich eines wirklich beendeten harmlosen
Node-Kindprozesses. Keine tatsächliche Audiowiedergabe ausgeführt.

Der native Watchdog läuft unabhängig mit gebundener Kernel-Prozessidentität.
Startwartezeit120s, Aufnahme330s, Verarbeitung120s, frische Berichte und
Gesamtlaufzeit sind begrenzt. Vor jedem TERM/KILL wird die Identität erneut
geprüft; Metadatenfehler allein sind kein Exitnachweis. Disarm ist nur bei
frischem Idle ohne Sitzung möglich. Fünf fokussierte Tests und fünf echte
CLI-Fälle mit eigenen harmlosen Kindprozessen bestanden: Disarm ohne Kill,
Abort, verschwundener Elternprozess, vorzeitiges Disarm und veralteter
Beobachter. Kein erzwungener PID-Reuse und kein echter Mikrofonlauf behauptet.

Runner, Wiedergabe und Watchdog unabhängig gelesen; gefundene Fehler
korrigiert und nachgeprüft. Der Watchdog-Vertrag gehört nun zum autonomen
Vollprofil; elf Schedulerverträge bestanden. Zielgerichtete Format-/Lint-
Prüfungen bestanden. Der unveränderte ASR-Pfad braucht deshalb keinen neuen
Korpuslauf. Der aktualisierte Lockfile-Pin dokumentiert ausschließlich drei
zusätzliche bestehende Dev-Abhängigkeitskanten; Entfernen dieser Kanten
reproduziert den vorigen Lockfileinhalt bytegenau.

Ein normales Produktbundle ist mit vorhandener lokaler Identität gebaut und
geprüft; seine Designated Requirement stimmt mit der installierten App
überein. Kein LSEnvironment und kein Entwickler-Steuerkanal aktiviert.
Das Bundle und ein eigener Empfänger sind vorbereitet, noch nicht installiert
beziehungsweise gestartet. Die tatsächliche Adapterintegration, vollständige
Vollsuite am zusammengeführten Stand, echte Bereitschaft und der einzige
physische Start bleiben offen. Der reale Gestenvermerk wurde nicht erzeugt.
