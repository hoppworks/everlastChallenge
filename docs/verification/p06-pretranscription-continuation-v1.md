# P06 – prospektive kontrollierte Übernahme und Restmessung

Stand: 18.10.2026. **PROSPEKTIV / NOT RUN / NOT QUALIFIED.**
Enge Ergänzung zum [Screeningprospekt](p06-pretranscription-screening-v1.md).
Dieser Text startet keinen Prozess und bestätigt keine Ausführungsbereitschaft.
Die unabhängige Codeprüfung ist abgeschlossen; ein neuer unveränderlicher Freeze und der tatsächliche vollständige Preflight stehen vor Ausführung getrennt an.

## Neue Auswertung über getrennte Ausführungszeiträume

Das hier definierte **Screening-Dataset** übernimmt bereits vollständig gültige
Fälle aus v2 und misst ausschließlich die fehlenden Fälle neu. Dies weicht für
diesen Datensatz von der bisherigen Regel ab, nach einem Fehler den Rest nicht
auszuführen.
Es ist keine automatische Wiederholung und keine nachträgliche Heilung von v2.
Historische Aussagen über fehlgeschlagene Versuche und ihre damaligen Regeln
bleiben unverändert; diese Ausnahme gilt nur für den neu eingefrorenen
Übernahmeauftrag und genau die unten bezeichneten 185 ganzen Fälle.

Der [v2-Fehlerbericht](p06-pretranscription-screening-v2-failed.md), sein JSON,
der v2-Freeze und sämtliche privaten Rohdaten bleiben unverändert. Insbesondere
bleiben der v2-Gesamtstatus `completed=false`, Exit 1, der nachträgliche
Akku-Befund des globalen Falls 186 und dessen beide ungültigen Screeningzeilen
erhalten. Andere Ausführungen, insbesondere v3, gehören nicht zum Übernahmepool.

Die logische ABBA-Ordnung und alle 100 Fälle bleiben fest:

| Logischer Block | Verbindlich aus v2 übernommen | In neuer Epoche auszuführen |
|---|---:|---:|
| B1 Reference | Fälle 1–100 | keine |
| B2 Pretranscription | Fälle 1–85 | Fälle 86–100 |
| B3 Pretranscription | keine | Fälle 1–100 |
| B4 Reference | keine | Fälle 1–100 |

Damit werden alle 185 gültigen **ganzen** Fälle mit ihren 370 Zeilen übernommen.
215 neue Fallprozesse liefern 430 neue Zeilen. Zusammen ergeben sie genau
400 logische Fälle und 800 Zeilen. B2/Fall 86
`constructed-de-masked-300s` wird einschließlich Warmup und Messlauf vollständig
neu ausgeführt; seine alten beiden pipelinegültigen, aber am Strom-Gate
ungültigen Zeilen werden nicht verwendet. Ein einzelner alter oder neuer
Warmup/Messlauf darf nicht durch den jeweils anderen ersetzt werden.

Jeder Fall hatte beziehungsweise erhält einen frischen Prozess, genau einen
Warmup und einen Messlauf. Auswahl und Übernahme erfolgen vor einer
Qualitätsansicht ausschließlich anhand der vollständigen Pipeline-, Strom-,
Identitäts- und Cleanup-Belege. Keine Auswahl nach Text, WER, Geschwindigkeit
oder günstigerem Ergebnis. Ein fehlgeschlagener neuer Fall macht den Rest der
neuen Epoche ausdrücklich unavailable; keine weitere automatische Übernahme
oder Wiederholung wird durch diesen Prospekt autorisiert.

## Unveränderte Rechen- und Eingabeverträge

Der neue Freeze bindet v2-Freeze, globalen Abschluss, Fehlerarchiv und jeden
übernommenen Fallabschluss samt Rohdateien mit vollständigen Hashes, Größen
und privaten Dateimodi. Übernahme allein aufgrund eines kopierten `ok`-Flags
reicht nicht: die ursprüngliche gültige Pipeline und AC-Beobachtungen werden
gegen die unveränderten Originalbelege nachgeprüft. Die feste Auswahl,
Referenzhashes und alle 100 Audiodateien bleiben identisch.

Binary, alle 35 kompilierten Quellpins, Modell/VAD, vollständige Decoder- und
Segmentparameter sowie Publisher-/Jitterregeln müssen mit v2 übereinstimmen.
Vorabtranskription bleibt unverändert: bestätigte 500-ms-Pause,
kontinuierlicher DSP, unveränderte Bereichs-, Besitz-, Join- und Abschlussregeln.
Kein Flash Attention, reduzierter Kontext oder anderer Backendwechsel.

Alte Fälle behalten ihre tatsächliche ursprüngliche Source-Revision,
Prozessdaten, Zeitstempel und Rohhashes. Neue Fälle weisen die neue
Launcher-/Epochenprovenienz aus. Ein neuer Launcher darf keine alte Sidecar
umschreiben oder ihre Source-Revision durch die neue ersetzen. Ein eigener,
frischer Namespace hält neue Rohdateien und Abschlüsse von v2/v3 getrennt.
Ein gewöhnlicher frischer Epochenfreeze darf alle 400 logischen Zielnamen
vorsehen; der Fortsetzungslauncher startet davon ausschließlich die festgelegten
215 neuen Prozesse. Die 185 übernommenen Fälle erhalten keine erfundenen neuen
Rawdateien, Prozessstarts oder Messwerte. Eine neue Zuordnungs-/Abschlussdatei
kennzeichnet ihre ursprüngliche Herkunft ausdrücklich als übernommen.
Das neue Dataset enthält nur Verweise und eine inhaltfreie Zuordnung jeder
logischen Zeile zu genau einer ursprünglichen Belegzeile; keine Originaldatei
wird ersetzt. Hashbindungen und die festgelegte Übernahmemenge werden vor dem
ersten neuen Modellprozess geprüft und eingefroren.

PID-Zahlen sind epochenübergreifend keine eindeutigen Prozessidentitäten und
können vom Betriebssystem wiederverwendet werden. Epochenkennung, gebundene
Ausführung und tatsächliche Start-/Enddaten bleiben zusammen erhalten.
Gleichzeitige Besitzerkonflikte dürfen nicht durch bloßen Zahlenvergleich mit
historischen PIDs erfunden oder übersehen werden. Neue Prozesse dürfen erst
nach bestätigt beendetem aktivem Vorgängerlauf beginnen; dieser Text erteilt
keinen Auftrag, einen laufenden Prozess zu stoppen.
Bleibt beim unterbrochenen v3-Fall die unmittelbare Beendigung unbestätigt,
bleibt dessen `terminationConfirmed=false` unverändert. Ausschließlich eine
zusätzliche unabhängige, an den tatsächlichen v3-Freeze und Abschluss gebundene
Besitzerfreigabe samt frischer Prüfung der betroffenen PID-/Prozessgruppenabwesenheit
darf die Startbarriere später schließen. Diese spätere Evidenz macht weder den
historischen Sofortnachweis erfolgreich noch irgendeine v3-Zeile übernehmbar.

## Bereitschaft und Fehlerbelege

Vor jedem neuen Modellstart gelten die bereits geprüften diskreten AC-Gates:
frischer Anfangs-/Startup-Check, Vorher-/Nachher-Beobachtung je Fall und
AC-Hostbeobachtung der Probe vor Modellinitialisierung. Akku, fehlende Evidenz,
Timing-/Tracefehler oder unbestätigtes Besitzerende verhindern weitere Starts;
bisherige Belege bleiben erhalten. Die Bedienperson bestätigt Netzanschluss,
offenen Deckel und ausreichende Verfügbarkeit. Kein kontinuierlicher
Strom-/Deckelschutz wird behauptet; keine globale Energieeinstellung ändern.

Die neue Ausführung schreibt privat, begrenzt, atomar und ohne Überschreiben.
Auch Fehler, unvollständige Rohdaten und nicht ausgeführte logische Slots bleiben
identifizierbar. Pipelinebeweise behalten vollständige Publikations-/Jobdaten
sowie die bereits geprüften Post-Join- und Strombeobachtungen. Die neue globale
Vollständigkeit bedeutet ausschließlich belegte Abdeckung der 800 logischen
Zeilen über die angegebenen Epochen, niemals einen ununterbrochenen Lauf.

## Unveränderte Qualitätsprüfung und enge Aussagegrenze

Nach vollständigem Readback entstehen acht kanonische Unicode-13-Zahlenberichte:
pro logischem Block je Warmup und Messung. Jede Zeile stammt exakt aus ihrer
vorab festgelegten Quelle; der Bericht bindet auch diese Herkunft. Die Referenzen
werden nicht verändert. Original90 und Zusatz10 sowie Warmup und Messung bleiben
getrennt. Paare 1/2 und 4/3 werden jeweils separat beurteilt:
`200 * (CandidateEdits - ReferenceEdits) <= ReferenceWords`, clean zusätzlich
`10 * CandidateEdits <= ReferenceWords`. Keine Nenneränderung, Poolbildung,
Auswahl einer besseren Phase oder Abschwächung von Stille-/Kurz-/Ankerkriterien.
Ganze geänderte Textpaare und Anker werden gesondert geprüft; Zahlen allein
begründen keine semantische Freigabe.

Dies ist ausschließlich ein Pipeline-/Qualitätsscreening über mehrere Epochen.
Unterbrechungen und zwischenzeitliche, nicht übernommene Läufe werden offengelegt.
Es ist kein zusammenhängender ABBA-Leistungsversuch: keine gemeinsame
L-/RTF-Leistungsstatistik, kein Ressourcen- oder P06-Qualifikationsschluss und
keine Produktionsaktivierung. Ein positives numerisches oder semantisches
Screening ersetzt nicht die weiterhin erforderlichen vollständigen
Latenz-, Ressourcen-, Langlauf-, Abbruch-/Neustart- und Produktabnahmen.

## Vor Ausführung zu protokollieren

Der eigene Ausführungsnachtrag nennt den ausdrücklichen Fortsetzungsauftrag,
diesen Prospekt, den akzeptierten Launcher-/Teststand und den neuen Freeze.
Er hält die festgelegten 185/370 übernommenen und 215/430 neuen Einheiten,
den ausschließlich gültigkeitsbasierten Auswahlzeitpunkt vor Qualitätsansicht,
den neuen Ausgabe-Namespace und den bestätigten Besitzerabschluss des zuvor
aktiven Laufs fest. Frische AC-/Bedienbereitschaft und tatsächlicher Start
werden erst nach ihrer Beobachtung eingetragen. Hier ist noch kein solcher
Start oder Erfolg protokolliert.
