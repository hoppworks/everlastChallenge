# Vergleich mit Voicely: Ergebnis und Aussagegrenzen

Der [native App-Vergleich vom 22.10.2026](verification/native-comparison-2026-10-22.md)
misst einen neueren Kandidaten bis zum beobachteten Textempfang. Sein Ergebnis
ist gemischt; ein allgemeiner Speedup ist nicht belegt. Der folgende historische
Helfervergleich bleibt davon getrennt.

## Historischer Helfervergleich

**Kurzfassung:** In einem historischen lokalen Datei-/CLI-Vergleich vom
20.10.2026 benötigte der Everlast-Kandidat pro Fall **36,4–39,1 % weniger
Prozesszeit** als der als Voicely 0.4.27 erfasste lokale Helfer. Das entspricht
einem Verhältnis der gemessenen Mediane von **1,57–1,64**.
Diese Messung ist **kein Geschwindigkeitsnachweis der beiden vollständigen Apps**
und kein neuer Benchmark des aktuellen Releases.

## Was gemessen wurde

Drei öffentliche deutsche FLEURS-Testaufnahmen mit 5, 10 und 15 Sekunden WAV-Länge.
Pro Aufnahme und Seite wurden fünf neue Prozesse gestartet: insgesamt 30 Läufe,
seriell in wechselnder Reihenfolge. Alle liefen erfolgreich bis zum Ende;
kein fehlgeschlagener Slot wurde aus dieser Serie entfernt oder ersetzt.

Die primäre Kennzahl ist die extern gemessene Zeit **vom Prozessstart bis zum
Prozessende**. Sie umfasst Modellprüfung und -laden, Erkennung, Schreiben des
Ergebnisses und Aufräumen. Modelle wurden nicht in einem residenten Dienst
vorgehalten. Betriebssystem-Dateicaches wurden jedoch **nicht geleert**.
„Neuer Prozess“ bedeutet deshalb nicht „kalter Datenträgercache“ oder
„erster Start nach der Installation“.

| Aufnahme       | WAV-Länge | Everlast Median | Vergleichshelfer Median | Weniger Zeit | Verhältnis |
| -------------- | --------: | --------------: | ----------------------: | -----------: | ---------: |
| FLEURS de 1736 |       5 s |          720 ms |                 1137 ms |       36,7 % |      1,58× |
| FLEURS de 1663 |      10 s |          759 ms |                 1247 ms |       39,1 % |      1,64× |
| FLEURS de 1668 |      15 s |          751 ms |                 1182 ms |       36,4 % |      1,57× |

Berechnung je Fall: `100 × (1 − Median Everlast / Median Vergleichshelfer)`.
Das Verhältnis ist `Median Vergleichshelfer / Median Everlast`. Die Tabelle
rundet nur zur Darstellung; gerechnet wird mit den veröffentlichten Einzelwerten.
Es gibt keinen aus den drei Fällen abgeleiteten universellen „Speedup“.

## Streuung und Textqualität

Die Everlast-Einzelwerte lagen bei **689–1002 ms**, **734–849 ms** und
**743–886 ms**. Alle drei Mediane lagen unter 800 ms; einzelne Läufe ausdrücklich
nicht. Fünf Wiederholungen je Fall sind eine kleine Stichprobe. Es werden keine
belastbaren p95-, Konfidenzintervall- oder Signifikanzbehauptungen abgeleitet.

Alle zehn Transkripte je Aufnahme waren zwischen beiden Seiten und über die
Wiederholungen bytegleich. Gleichheit bedeutet hier nicht Fehlerfreiheit:
Die drei verschiedenen Aufnahmen enthalten zusammen 55 Referenzwörter und
sieben Wortfehler, also **12,73 % WER** nach dem vorhandenen Unicode-13-Evaluator.
Die Wiederholungen erzeugen keine zusätzlichen unabhängigen Sprachbeispiele.
Insbesondere bleiben ein Namensfehler und weitere Schreibabweichungen erhalten.
Es wurde keine bessere Erkennungsqualität als beim Vergleichspartner nachgewiesen.

## Genaue Reichweite

- Gemessen wurde ein **historischer Everlast-Kandidat**, Quellrevision
  `2ded384b7a8eb2136db9544d6d60b14c84fda07d`, mit MLX-Encoder,
  whisper.cpp-Decoder und aus Q5 abgeleiteten F32-Kontrollgewichten. Der aktuelle
  Standard verwendet FP16-Gewichte. Diese Ergebnisse werden nicht nachträglich
  zu einer Messung des aktuellen Modellstandes erklärt.
- Der Vergleichspartner ist der lokal erfasste Datei-/CLI-Helfer, dessen
  Binärhash im Datensatz steht. Die Messung belegt weder dessen Gleichheit mit
  dem vollständigen Voicely-GUI-Ablauf noch die Leistung anderer Versionen,
  Abos, Modelle oder Einstellungen. Sie wird nicht als offizielle Messung
  oder Bestätigung durch den Hersteller dargestellt.
- Gleiche Audiodateien und ein gemeinsamer äußerer Timer verbessern die
  Vergleichbarkeit. Modellrepräsentation, VAD, Threads und Implementierung
  sind nicht experimentell auf identische Einzelparameter reduziert. Das
  Ergebnis beschreibt die verglichenen Konfigurationen, keine isolierte
  Überlegenheit einer einzelnen Bibliothek oder GPU-Operation.
- Die internen Timer nach dem Laden haben nicht nachgewiesen identische Grenzen.
  Daraus wird keine zusätzliche Prozentzahl als Hauptaussage abgeleitet.
- Ein lokaler Prüfhost und drei kurze deutsche Dateien belegen keine Ergebnisse
  für andere Macs, längere Aufnahmen, Englisch, Akzente, Nebengeräusche oder
  Stille. Die bekannten Fälle wurden während der Optimierung mehrfach benutzt;
  es handelt sich nicht um ein blindes oder unangetastetes Testset. Dargestellt
  wird diese abschließende Serie, nicht die Gesamtheit aller Entwicklungsversuche.
  Es gibt keinen Energie- oder Speichervergleich aus dieser Serie.
- Mikrofonaufnahme, Hotkey-Verarbeitung, Fokuswechsel, Einfügen und tatsächlicher
  Textempfang in einer Zielanwendung wurden **nicht** in dieser Zeit gemessen.
  Die separaten nativen Funktionstests ersetzen diesen Leistungstest nicht.

## Nachrechnen und Nachweisgrenze

Die [30 veröffentlichten Messzeilen](benchmarks/helper-comparison-2026-10-20.json)
enthalten Zeit, Fall, Seite, Wiederholung, Abschlussstatus und Transkript-Hash.
Pfade, Prozess-IDs, interne Arbeitsunterlagen und fremde Binärdateien gehören
nicht zum öffentlichen Datensatz. Die Hashes der Originalzeilen und der
ursprünglichen Messkonfiguration erhalten die lokale Nachweiskette. Dieser
Auszug ist ausdrücklich kein unverändertes vollständiges Roharchiv.

```sh
node --import tsx tools/measurement/summarize-helper-comparison.ts
```

Das Skript prüft Vollständigkeit und Transkriptgleichheit und berechnet Mediane,
Spannweiten und Verhältnisse erneut. Es reproduziert die **Auswertung**, nicht
die Erkennung oder die damals verstrichene Zeit. Ohne damalige Binärdateien,
Modelle und Umgebung ist kein identischer Benchmarklauf versprochen.

Für eine belastbare Behauptung „unsere aktuelle App ist schneller als Voicely“
reicht auch der neue begrenzte Kandidatenvergleich nicht aus. Nötig wären beide
unveränderten Release-Apps: dieselbe
Audiozuführung, getrennte Erststart-/Warmstartreihen, vorab definierte Fälle und
Wiederholungen, Zeit vom Aufnahmeende bis zum tatsächlich sichtbaren finalen
Text sowie eine parallele Qualitätsauswertung. Die Aussagen bleiben auf die jeweils beschriebenen Messreihen begrenzt.
