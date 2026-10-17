# P05: Quellen für einen unabhängigen Korpuszusatz

Stand: 17.10.2026. Die ursprüngliche Recherche erfolgte ausschließlich an öffentlichen Primärseiten,
HTML-/JSON-Metadaten und Textquellen. **Während dieser Recherche kein Audio
heruntergeladen oder angehört, keine Inferenz, kein Modell, keine Änderung
bestehender Manifeste.** Die spätere tatsächliche Beschaffung ist im Nachtrag
unten getrennt dokumentiert. Die unten
vorgeschlagenen Intervalle sind noch keine fertigen Goldreferenzen oder
P05-Abnahmefälle. Insbesondere ist natürliche Lesesprache nicht gleichbedeutend
mit fünf Minuten ohne sichere Sprechpause.

## Empfehlung: Kafka, Das Urteil, Teil II

Eine konkrete menschliche deutsche Lesung mit ausdrücklich dokumentierter
Audiolizenz ist **„Franz Kafka – Das Urteil Teil II“**, gelesen von
**Hans-Jörg Große**, Produktion **Christian Mantey und Hans-Jörg Große**, Berlin,
November 2008. Die vom beteiligten Account Kulturagent veröffentlichte
[Commons-Dateiseite, feste Revision 1186942896](https://commons.wikimedia.org/w/index.php?title=File:Franz_Kafka_-_Das_Urteil_Teil_II.ogg&oldid=1186942896)
nennt **440,372244897959 s**, **6.528.127 Byte**, Ogg Vorbis und den Metadaten-SHA-1
**`0a3562d71ee074a6dde7ac4512ceb136c91d26be`**. Dies sind Publishermetadaten,
keine lokal geprüften Audioeigenschaften. Für eine spätere Beschaffung ist der
[Originaldatei-Link](https://upload.wikimedia.org/wikipedia/commons/1/12/Franz_Kafka_-_Das_Urteil_Teil_II.ogg)
konkret vorhanden; er wurde nur aus HTML gelesen, nicht abgerufen.

Die Dateiseite dokumentiert die ausdrückliche Freigabe der Aufnahme unter
**CC BY-SA 3.0 Unported** sowie eine Lizenzprüfung beider ursprünglicher
Rechteinhaberseiten vom 06.10.2012. Nur diese belegte 3.0-Freigabe verwenden;
die Seite relativiert die Nachweislage ihrer weiteren angebotenen Lizenzen.
Die ursprünglichen Anbieterlinks sind
[Hans-Jörg Große](https://www.hans-joerggrosse.de/das_urteil/) und
[Kulturagent](http://kulturagent.eu/erlesenes/franz_kafka/urteil/).
Die Sprecherseite ließ sich in dieser Recherche nicht mehr erfolgreich lesen;
der historische Nachweis ist auf Commons dokumentiert.

### Audio- und Textrechte für Deutschland getrennt

Für die Aufnahme wird keine Gemeinfreiheit aus Kafkas Todesjahr abgeleitet:
Neue Darbietungen und Tonträger besitzen eigene Rechte, siehe
[§82 UrhG](https://www.gesetze-im-internet.de/urhg/__82.html) und
[§85 UrhG](https://www.gesetze-im-internet.de/urhg/__85.html).
Nutzungsbasis ist die dokumentierte CC-Freigabe. Der
[CC-BY-SA-3.0-Lizenztext §§1,3,4](https://creativecommons.org/licenses/by-sa/3.0/legalcode)
erfasst Darbietungen/Tonträger und erlaubt weltweite Vervielfältigung und
Bearbeitung unter seinen Bedingungen. Bei Weitergabe des Ausschnitts Namen,
Werk, Quelllink und Lizenz beilegen; Zeitbeschnitt, Mono-/16-kHz-Konvertierung
und etwaige weitere Änderungen nennen und die bearbeitete Fixture unter
CC BY-SA 3.0 weitergeben. Keine ausschließlichen Rechte oder Unterstützung des
Projekts durch die Sprecher behaupten.

Eine unabhängige öffentliche Schriftquelle ist
[Wikisource: Das Urteil, feste Revision 2572967](https://de.wikisource.org/w/index.php?title=Das_Urteil&oldid=2572967).
Sie weist den deutschen Originaltext aus *Arkadia* (1913), Franz Kafka als Autor,
keinen Übersetzer und zweimalige Korrektur anhand der Vorlage aus. Der Text
existierte vor dieser Aufnahme. Kafka starb 1924; für seinen bereits 1913
veröffentlichten Originaltext folgt aus
[§64 UrhG](https://www.gesetze-im-internet.de/urhg/__64.html) und
[§69 UrhG](https://www.gesetze-im-internet.de/urhg/__69.html) rechnerisch das Ende
der regulären deutschen Schutzfrist mit Ablauf 1994. Das ist eine Einordnung
des Originaltexts, keine pauschale Freigabe moderner Editionen, Kommentare oder
Übersetzungen. Max Brod wird als Herausgeber der Sammlung genannt; keine seiner
Zusätze als gesprochenen Kafka-Text übernehmen. Die Audio-Sammlung verweist auf
die Ausgabe 1916, die Textseite auf 1913: Editionsgleichheit und tatsächlich
gesprochene Wörter sind vor einer Goldreferenz noch unabhängig abzugleichen.

### Vorabverfahren für 300 s und 30 s

Vorschlag für eine spätere, vor jedem Modelllauf festzuschreibende Auswahl:

1. Genau das oben genannte Original beschaffen, Größe/SHA-1 prüfen und zusätzlich
   SHA-256 sowie ursprüngliches Format festhalten. Keine Kombination der Teile
   I–IV und keine Entfernung innerer Pausen. Dekodierung/Resampling mit
   Werkzeugversion, Binärhash und Sample-Rundung dokumentieren.
2. Als ersten fest bestimmten Kandidaten **Originalzeit [30,330) s** vorsehen:
   300 s zusammenhängende Aufnahme, bei 16 kHz genau 4.800.000 Frames. Vor ASR
   unabhängig anhören, um Titelansage, Musik, Schnitte sowie angeschnittene
   Anfangs-/Endphoneme zu erkennen. Besteht dieser Kandidat nicht, keinen
   passenden Modelloutput zur Ersatzwahl verwenden. Ein neues ausschließlich
   signal-/referenzbasiertes Auswahlverfahren separat vorab festlegen.
3. Am unveränderten Kandidaten die eingefrorene produktive Regel für eine
   sichere Pause prüfen, insbesondere 20-ms-Fenster, Energiehysterese und
   500-ms-Bestätigung. Maximale erkannte Pausenlänge und Parameter berichten.
   Wenn eine sichere Pause existiert, als natürlichen Langfall mit Pausen
   klassifizieren; **nicht** „ohne sichere Pause“ behaupten und keine Pausen
   mit Rauschen maskieren. Der gesuchte strengere Fall bleibt dann offen.
4. Zwei unabhängige menschliche Durchgänge ordnen die gesprochenen Wörter dem
   veröffentlichten Text zu und markieren Grenzen/Abweichungen. Referenz aus
   dieser Quelle plus dokumentierten Hörkorrekturen, nie aus Whisper gewinnen.
   Uneinigkeit vor ASR lösen; Zeitspanne, Referenz, Anker und Hashes einfrieren.
5. Ein zusätzlicher Kandidat **[30,60) s** ergibt exakt 480.000 Frames, bleibt
   aber wegen Überlappung mit dem Langfall **kein unabhängiger Satz**. Dieselbe
   Grenz-/Referenzprüfung anwenden. Für einen unabhängigen 30-s-Abnahmefall einen
   anderen unverbrauchten Abschnitt/Sprecher vorab festlegen. Weder Zeitdehnung
   noch TTS, Verkettung oder Textableitung aus Modellhypothesen verwenden.

Diese Methode liefert eine überprüfbare Auswahlentscheidung, keine Behauptung,
dass die Metadaten bereits Pausenfreiheit oder passende Wortgrenzen beweisen.
Eine Zeitreferenz für den Ausschnitt ist noch nicht vorhanden.

## Konkrete LibriVox-Alternative

[LibriVox: Die Verwandlung](https://librivox.org/die-verwandlung-von-franz-kafka/)
nennt **Markus Wachenheim**, Deutsch, Katalogdatum 23.11.2008 und für
**Abschnitt 1** **41:05**. Der
[Kapitel-Originaldownload](https://archive.org/download/verwandlung_mw_0811_librivox/verwandlung_01_kafka.mp3)
ist vorhanden. Die gelesenen
[Archive-Metadaten](https://archive.org/metadata/verwandlung_mw_0811_librivox)
weisen dafür **39.452.911 Byte**, **2465,8 s**, SHA-1
`b8b2da421276270c8f1d1b2a041185dbeb0ba675` und die alte Creative-Commons-
Public-Domain-Dedication-URL aus. Kein Audio wurde abgerufen.

Als unabhängiger deutscher Originaltext ist
[Project Gutenberg 22367](https://www.gutenberg.org/ebooks/22367) verfügbar,
mit Kafka (1883–1924) als Autor und ohne Übersetzer. Seine konkrete Edition
muss ebenfalls zur Lesung abgeglichen werden; die Beschreibung des Buchinhalts
auf der Katalogseite ist keine Referenz. Für Deutschland gilt die oben begründete
Originaltext-Einordnung.

**Nicht mit einer globalen CC0-Lizenz verwechseln:**
[LibriVox' eigene Rechteerklärung](https://librivox.org/pages/public-domain/)
beschreibt eine Freigabe zur beliebigen Nutzung, grenzt ihre gesicherte
Gemeinfreiheitsaussage aber auf die USA ein und verlangt außerhalb der USA die
Prüfung des örtlichen Rechts. Der Archive-Eintrag nennt keine moderne CC0-
Fallbacklizenz. Deshalb ist die ausdrücklich CC-BY-SA-lizenzierte Aufnahme von
*Das Urteil* für den neuen lokalen Zusatz die engere Empfehlung. Die
LibriVox-Alternative ist ein konkreter Beschaffungskandidat, keine hiermit
abschließend festgestellte deutsche Gemeinfreiheit der Aufnahme.

## Natürliche Kurzsprache und A22

Die Einzelwortlabels stammen aus den ursprünglichen Metadaten, nicht aus ASR.
Die Wortreferenzen sind vor der Messung festzuschreiben. Keine Phoneme
abschneiden oder Sprachgeschwindigkeit verändern, um 0,2 s zu erzwingen.
Vorhandenes Heidelberg-Digits-Audio bleibt eine weitere Quelle für natürliche
Zahlwörter; es deckt Ja/Nein nicht ab, siehe
[bereits geprüfte Quellen](corpus-sources.md#hd-nachtrag-aus-dem-tatsächlich-vorhandenen-originalarchiv).

Für **A22 „Vielen Dank fürs Zuschauen“** wurde in dieser begrenzten Suche kein
zugleich natürlich menschlich gesprochenes, passend lizenziertes und unabhängig
referenziertes Audio belegt. Ein
[Powtoon-Treffer mit Text und CC-Hinweis](https://www.powtoon.com/online-presentation/daL9D5mj1va/dff/)
belegt weder menschliche Stimme noch eine hinreichend konkrete Audiolizenz und
ist daher **kein Kandidat**. Ein
[offizielles Euroquiz-Manuskript von SCILT](https://scilt.org.uk/Portals/24/Library/Euroquiz/Euroquiz%202025%20Final.pdf)
enthält den Satz, belegt in dieser Recherche aber keine offene Lizenz der
zugehörigen Tonaufnahme. Ebenfalls nicht verwenden. Am engsten wäre eine neue
menschliche, ausdrücklich freigegebene Einzelaufnahme dieses vorgegebenen
Satzes; eine eigene getrennte Kalibrierungs-/Abnahmeaufnahme vor jeder Inferenz
festhalten.

## Geprüfte weitere Option: Spoken Wikipedia Corpus (SWC)

Die [offizielle SWC-Projektseite](https://nats.gitlab.io/swc/) beschreibt
menschliche Wikipedia-Lesungen, mit dem ursprünglichen HTML verknüpfte
Textannotation sowie Wort-/Phonemzeitgrenzen. Die aktuelle Ausgabe ist laut
[primärem UHH-Repository, Version 2.0 vom 27.10.2017](https://doi.org/10.25592/uhhfdm.1875)
unter **CC BY-SA 4.0** verfügbar. Der dort konkret angebotene Vollbestand
`swc-2.0.zip` hat **37,6 GB**, MD5 `b6e8567e39cc0b44bc03b05e576fee69`;
Metadaten `swc-2.0.cmdi` und `swc-2.0.xml` sind gesondert ausgewiesen.
Keine dieser Aussagen belegt allein die ursprüngliche Lizenz einer bestimmten
Commons-Aufnahme oder deren konkrete Wikipedia-Revision.

Die [Autorenpublikation von Köhn, Stegen und Baumann (LREC 2016)](https://aclanthology.org/L16-1735/)
beschreibt die automatische Zuordnung vorhandener Wikipedia-Texte zu Audio und
weist ausdrücklich auch nur teilweise ausgerichtete Sätze aus. Das ist eine
**veröffentlichte automatische Ausrichtung eines unabhängigen Textes**, keine
Referenzgewinnung aus unserem Whisper-Kandidaten; es ist aber auch keine manuell
gehörte Wortgenauigkeit. Der
[Autorenbeitrag zum XML-Schema](https://arne.chark.eu/2017/xml-vs-json/)
unterscheidet Text, Normalisierung, Zeitannotation und nicht ausgerichtete Teile.
Für P05 wären insbesondere `ignored`-Bereiche, fehlende Zeiten, Zahlenexpansionen
und abweichende Wort-/Phonemgrenzen unabhängig zu prüfen. Die SWC-Normalisierung
nicht als zusätzliche Zahlumwandlung in den P05-WER-Vergleich übernehmen.

Im eng begrenzten Primärzugang wurde **kein konkret einzeln abrufbarer deutscher
Artikel mit zugleich belegter Dauer >=300 s, Dateigröße, Original-Audiolizenz,
genauer Textrevision und zugänglicher zugehöriger Alignmentdatei identifiziert**.
Die verlinkten Demo-/Verzeichniszugänge auf dem alten UHH-Server, Bitbucket und
die ZIP-Vorschau waren nicht erfolgreich zugänglich. Es wurde kein
Corpus-Audioarchiv geladen, kein breitflächiger Corpus-Scan vorgenommen und kein
erratener Dateipfad als Quelle ausgegeben. SWC ist deshalb eine methodisch
passende weitere Option, **noch kein zusätzlicher beschaffungsfertiger Kandidat**.
Vor einer Aufnahme ins Zusatzmanifest müsste genau ein Artikel samt
`info.json`/Sprecher, Original-Commons-Datei, Wikipedia-`oldid`, Alignment und
Hashkette einzeln belegt werden. Auch dann blieben sichere Pausen und die
Referenz des ausgewählten 300-s-Intervalls zu prüfen.

## Grenze des Ergebnisses

Gefunden sind ein konkreter lizenzierter Langaufnahme-Kandidat mit unabhängiger
Textquelle sowie zwei kurze Wortaufnahmen. **Nicht nachgewiesen** sind eine
bereits sauber ausgerichtete 300-s-Goldreferenz ohne sichere Pause, vollständige
Wortgrenzen im vorgewählten 30-s-Fenster, echte 0,2-s-Sprache und ein nutzbarer
öffentlicher A22-Audiobeleg. Neue Fälle gehören in ein separates eingefrorenes
Zusatzmanifest. Bisherige Abnahmefälle bleiben unberührt; Kalibrierung darf
keinen Ausschnitt mit der neuen Abnahme teilen. Ein zweiter Ausschnitt desselben
Sprechers/Werks ist nicht automatisch eine unabhängige Stichprobe.

## Nachtrag: tatsächliche Beschaffung und Signalprüfung

Am 17.10.2026 nach Abschluss der Quellenrecherche wurden ausschließlich die
drei konkret genannten Commons-Originale beschafft. Jeder Download war auf
die vorab bekannte Größe plus ein Kontrollbyte begrenzt; Größe und Publisher-
SHA-1 stimmen exakt überein. Die Dateien liegen lokal und ignoriert unter
`artifacts/corpus/supplement-source/`; keine Audiodaten werden committet.

| Originaldatei | Lokal geprüfter SHA256 |
|---|---|
| kafka-urteil-teil-ii.ogg | `846b0096e7e3ad71402a4cda8e063994b80f626bfa472754ca8f139d94d3e4b1` |
| de-ja.ogg | `47582f7a2b9663062630d13185b35879e75962bdf792bb565f0d60efcbee9442` |
| de-nein.ogg | `8d93672fa903c65771c21ed264397e547fd66af94c56a2e0b3646c3705b7dbed` |

FFprobe bestätigt für Kafka Ogg/Vorbis, Stereo, 44.100 Hz, 440,372245 s.
FFmpeg 9.0.2 (Binär-SHA256
`c8173e9755795978bce8e104f8d7044fabe7d6d84044aba37d3648c6f4e4e1a8`)
dekodierte mit `-map 0:a:0 -ac 1 -ar 16000 -c:a pcm_s16le`, ohne Filter
zum Entfernen von Pausen. Die vorab ausgewählten Intervalle wurden anschließend
als ganzzahlige PCM-Samplebereiche aus der vollständigen Dekodierung entnommen.

- [30,60) s: genau 480.000 Frames, WAV-SHA256
  `e3bc915ed6694d3b17d4cb50349f36fcca3e93fa070a73226d22017f0d6e0adc`.
- [30,330) s: genau 4.800.000 Frames, WAV-SHA256
  `4ee705e81daa398f38ce4da545a56c632458fd69fa189d480dbd75703b51536c`.
- Ja/Nein: vollständig dekodiert, 10.031 beziehungsweise 9.288 Frames;
  nicht auf 0,2 s verkürzt.

In beiden Kafka-Intervallen liegen null vollständige 20-ms-Fenster unter der
aktuellen Energie-Endschwelle 0,000025 RMS. Es gibt damit insbesondere keinen
500-ms-Lauf unter dieser Schwelle. Dies ist eine einfache Signaldiagnose,
keine abgeschlossene Segmentiererabnahme: Die Produktionsbasis besitzt derzeit
nur die äußere Energiebegrenzung; die vollständige Pausenbestätigung gehört
noch zu P06. Hintergrundmusik oder Umgebungspegel können die Schwelle ebenso
überschreiten wie Stimme. Keine hörgeprüfte Pausenfreiheit daraus ableiten.

Das lokale `inspection-v1.json` (SHA256
`c7ba2e45d6bf6321805d1b4217193279d1362709bfd9ccf5bcf46bdc8663db2b`) enthält vollständige Decode-Befehle, Original-/
WAV-Hashes, Samplezahlen, Intervalle und ausdrücklich falsche Flags für
ASR-Ausführung, Hörprüfung und eingefrorene Goldreferenz. Es gab weiterhin
keine Inferenz und keine Änderung bestehender Manifeste. Als nächster Schritt
bleiben zwei unabhängige Hör-/Textabgleiche samt Grenzwortprüfung und erst
danach das Einfrieren eines separaten Zusatzmanifests.
