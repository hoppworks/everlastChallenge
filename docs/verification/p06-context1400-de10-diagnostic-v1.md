# Audiokontext 1400 am bekannten deutschen Zehn-Sekunden-Fall

Begrenzte Datei-Diagnose am 19.10.2026, keine Korpus- oder Leistungsabnahme.
[Strukturierter Bericht](p06-context1400-de10-diagnostic-v1.json).

## Vorab festgelegt und ausgeführt

Der bereits unabhängig geprüfte, eingefrorene Diagnosebinary des
[Kontext-1400-Versuchs](p06-context1400-calibration-v1.json) wurde unverändert
wiederverwendet. Binary, beide Parameterdateien, ASR-/VAD-Dateien und öffentliche
WAV vor Ausführung gegen bekannte SHA256 geprüft. Kein neuer Build, kein
Mikrofon, keine UI- oder Produktänderung. Netzstrom vor/nach den Aufrufen.

Fall `fleurs-de-test-1663`, WAV-SHA256
`e86ac3cc846d6fb8655fbbdaa6906d8812810bcc03fedeb4c385b7d039d0b79d`.
Dieser Fall war bereits im fehlgeschlagenen ersten Leistungspaar exponiert;
keine neue Blindprüfung. Eingabe 160000 Frames, jeweils 159680 vorbereitete Frames.
Prospektiv eingefrorene Prozessreihenfolge: Referenz, 1400, 1400, Referenz. Jeder
Prozess ein Warmup und drei gemessene Aufrufe; insgesamt 16 Aufrufe. Alle vier
Prozesse Exit 0, validierte Profilidentitäten, Aufräumen/Workerjoin erfolgreich.
Keine Wiederholungen oder entfernten Messwerte.

## Beobachtung

| Block | Kontext | NativeFull-Median | EngineTotal-Median |
|---|---:|---:|---:|
| 1 | Referenz0 / Modell1500 | 1004,061 ms | 1032,786 ms |
| 2 | 1400 | 925,419 ms | 953,010 ms |
| 3 | 1400 | 923,622 ms | 950,952 ms |
| 4 | Referenz0 / Modell1500 | 1003,055 ms | 1029,678 ms |

NativeFull sinkt deskriptiv um 7,832 % beziehungsweise 7,919 %; EngineTotal um 7,724 %
beziehungsweise 7,646 %. Alle 16 Ausgaben einschließlich Warmups haben denselben
UTF-8-Texthash. Diese Gleichheit sagt etwas über diesen Fall, nicht über die
allgemeine Erkennungsqualität oder Korpus-WER.

Die Enginezeit liegt hier mit 1400 unter einer Sekunde. Sie enthält keinen
realen Stopzeitpunkt, keinen Capture-/Actor-Abschluss und keinen tatsächlichen
Empfänger: daher keine bestandene Stop-bis-Text-Grenze, kein L und kein P06-
Abschluss. Auch die eigenständige reproduzierbare 10 %-Bedingung wird mit diesem
Einzelfall nicht belegt. Keine Kombination mit Full-Prefix gemessen.

## Korrektur und Konsequenz

Der [frühere Kontext1400-/Kombinationspilot](p06-context1400-futility-2026-10-19.md)
war bereits vorhanden und wurde bei der Vorbereitung übersehen. Dieser DE10-
Dateipilot war deshalb vermeidbar; keine weitere Wiederholung daraus ableiten.
Der [Offline-Hashtransfer](p06-context1400-existing-content-bridge-v1.json)
verknüpft alle 16 Ausgaben exakt mit dem bereits bewerteten Referenz-/Kandidattext:
beide unverändert 4/21 Wortfehler. Der zugehörige DE15-Kandidat verschlechtert
sich dagegen von 2/24 auf 6/24. Eine neue Kombination oder allgemeine
Qualitätsneutralität darf aus diesem DE10-Ergebnis nicht abgeleitet werden.
Produktionsreferenz bleibt unverändert; P06 bleibt offen.

Unabhängiger Readback ohne Rechen-, Bindungs-, Identitäts- oder Kontextdelta-
Befund. Eine Beleggrenze: Die Reihenfolge ist durch Vorabfreeze, Completionliste,
unterschiedliche PIDs und aufsteigende Ausgabe-mtimes gestützt. Separate
Prozess-Start-/Endzeitstempel wurden nicht aufbewahrt; streng überschneidungsfreie
Ausführung ist aus diesen Artefakten allein nicht unabhängig bewiesen.
