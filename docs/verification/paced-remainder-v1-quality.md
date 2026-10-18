# P05: Qualität des 94-Fälle-Restblocks v1

Stand:18.10.2026. Referenzmodell `large-v3-turbo-q5_0`, Messcode `b9799b6`.
Alle94 Fälle wurden einmal warm und zehnmal gemessen:1034 erfolgreiche
Ausgaben, davon940 Messzeilen und94 separat ausgewertete Warmups. Dieser
Bericht prüft Qualität und Originalinhalt; Timing, Policy und Ressourcen werden
im strukturellen Laufbericht geprüft. Keine neue Inferenz oder Referenzänderung.

## Unveränderte Auswertung, getrennte Quellen

`/usr/bin/python3`3.9.6/Unicode13.0.0 führte den unveränderten
`scripts/quality.py.evaluate` elfmal mit jeweils94 Fällen aus: einmal Warmup,
zehnmal je Messindex. Ergebnisprojektion ausschließlich `{id,status,text}`,
Slot-IDs eindeutig. Kein1034-Fälle-Aufruf und keine Erhöhung des500er-Limits.
Zusätzlich wurden innerhalb jedes Index die vier Quellfamilien getrennt
berechnet. Die mechanischen Gesamtgruppen des94er-Aufrufs sind im JSON zur
Reproduktion erhalten, dienen aber keiner gemischten Korpusqualifikation.

Je Fall sind alle elf Hypothesen bytegleich. Sämtliche94 Texte stimmen zudem
exakt mit ihren jeweiligen historischen Native-Timestamp-Referenzausgaben
überein. Es gibt94 Eingabefall-Paare, aber wegen Quietvarianten, Emptykontrollen
und gleicher Texte nur79 unterschiedliche vollständige Textpaare. Alle wurden
vollständig gelesen; Wiederholungen erzeugen keine neue lexikalische Abdeckung.

Die folgende Tabelle gilt **für jeden einzelnen Messindex**. Die separate
Warmup-Auswertung hat dieselben Werte. Beim Summieren der zehn Messindizes
werden Zähler und Nenner jeweils mit10 multipliziert; unabhängige Referenzwörter
entstehen dadurch nicht.

| Quellfamilie / Gruppe | Fälle pro Index | Edits / Referenzwörter | WER |
|---|---:|---:|---:|
|Original84, DE clean|40|39/860|4,5349%|
|Original84, EN clean|29|33/679|4,8601%|
|Original84, DE challenging|8|230/559|41,1449%|
|Original84, EN challenging|7|5/99|5,0505%|
|Supplement, DE30s|1|4/32|12,5000%|
|Supplement, EN30s|1|4/52|7,6923%|
|Supplement, drei synthetische DE-Kalibrierfälle|3|0/6|0%|
|Kalibrierrollen, DE Nachlauf plus Room|2|0/26|0%|
|Kalibrierrollen, EN Nachlauf plus Room|2|2/21|9,5238%|
|Separater controlled300, DE challenging|1|28/463|6,0475%|

Die Original84 enthalten vier Emptyfälle; die Kalibrierrollen zwei weitere.
Alle sechs liefern in allen elf Ausgaben exakt leeren Text und gehen nicht in
WER-Nenner ein. Die zwei Nachlauffälle enthalten keinen zusätzlichen Endtext.
Beim englischen Nachlauf entstehen die zwei WER-Edits durch die unverändert
bewertete Abkürzungsdarstellung; die vollständige Aussage bleibt erhalten.

Die zentralen sechs Fälle und ihre16 Inventaranker sind aus den94er-Indizes
ausgeschlossen. Ihr [separater v4-Qualitätsbericht](paced-six-reference-v4-quality.md)
bleibt unverändert. Die Wiederholungsgewichte wurden nicht vermischt.

Eine **zusätzliche ungewichtete Full90-Rekonstruktion** nimmt genau eine
tatsächliche Messhypothese je ursprünglichem Fall:84 Restfälle `measured-001`
und jeweils die erste Nicht-Warmup-Ausgabe der sechs Zentralfälle, in der
unveränderten Reihenfolge von `acceptance-v1.json`. Decoder-Runtime,
Vorbereitung und Stateparameter beider Parameterdateien sind exakt gleich.
Der Evaluator erhält90 eindeutige ursprüngliche IDs, keine Wiederholungen:
DE clean46/915=5,0273%, EN clean34/734=4,6322%; challenging DE230/559 und
EN5/99 bleiben separat. Das reproduziert die historischen Full90-Werte,
ohne neue Inferenz, neue Blindheit oder Latenz-/Ressourcengewichtung.

## Originalinhalt und Anker

Alle94 vollständigen Fallpaare wurden auf Zahlen, Negationen, Entitäten,
Satzteilverlust, Dopplung und Zusatztext gelesen. Bei identischen Paaren sind
historische clean-Ankerurteile unverändert mit Datei- und kanonischen
Urteil-SHAs gebunden; Ganzsatzfehler bleiben auch ohne Inventaranker sichtbar.
Namensorthografie und tatsächliche Entitäts-/Wortsubstitution werden getrennt.
Es gibt keine neue pauschale Null-Namensfehlergrenze.

| Quellfamilie | Eindeutige Fallurteile pass / fail / unclear | Gemessene Zeilen pass / fail / unclear |
|---|---:|---:|
|Original84|68 /15 /1|680 /150 /10|
|Supplement5|3 /2 /0|30 /20 /0|
|Kalibrierrollen4|4 /0 /0|40 /0 /0|
|Controlled300|0 /1 /0|0 /10 /0|

Zusammen sind es75 pass/18 fail/1 unclear unter94 Eingabefällen bzw.
750 pass/180 fail/10 unclear unter940 Messwiederholungen. Die94 Warmups bleiben
separat mit75/18/1 ausgewiesen. Die wiederholten Fehler sind keine180 neuartigen
Fehler. Alle unveränderten historischen Urteile und ihre Grenzen bleiben erhalten.

Die69 clean-Fälle des Originalrests enthalten142 der158 historischen
Inventaranker:18 Negationen pass;33 Zahlenanker pass;60 Entitätsanker pass,
zwei fail und einer unclear;28 Zweifelsanker pass. Die zehn Quietvarianten
verwenden26 Anker aus identischen Originalpaaren erneut; diese sind keine
zusätzlichen unabhängigen Inventaranker. Mit Quietvorkommen sind es168
Ankervorkommen pro Index:165 pass, zwei fail, einer unclear. Diese Ankerzahlen
ersetzen keine Volltextprüfung der übrigen Fälle und insbesondere der300s-Fälle.

Konkrete bekannte Befunde bleiben bestehen:

- `constructed-de-masked-300s`:225/463 Edits,470 Hypothesentokens. Zahlreiche
  lokale Wort-/Entitätsschäden, falsche numerische Inhalte, beschädigte
  Negationen, fehlende Aussagen und unbelegter Zusatzinhalt; terminale
  Entitätswiederholung. Weiterhin fail, kein Nachweis vollständiger Texttreue.
- `diagnostic-de-controlled-300s`:28/463 Edits,467 Hypothesentokens. Alle21
  Quellblöcke einmal erkennbar, aber falsches Handlungsverb in der Anweisung,
  beschädigte zitierte Begriffe und Namen sowie zusätzliches Endwort.
  Keine ganze Blockauslassung oder -dopplung beobachtet; trotzdem fail.
- Die zwei30s-Supplementfälle bleiben fail für vollständige Texttreue:
  DE mit zusätzlichem terminalen Bindewort bei erhaltenen Zahlenrelationen;
  EN mit falschem technischen Eigenschaftswort bei erhaltener Negation,
  Namensschreibabweichung und ausgelassenem redundantem Alias.
- Bestehende lokale Wort-/Entitätsschäden der kurzen Originalfälle bleiben
  in den fallweisen Urteilen dokumentiert. `fleurs-en-test-1669` bleibt
  hinsichtlich Namensidentität unclear. Die zwei fehlerhaften englischen
  Quietvarianten behalten dieselben semantischen Fehler wie ihre Originale.

Der [inhaltfreie JSON-Bericht](paced-remainder-v1-quality.json) enthält alle
1034 numerischen Bewertungen,94 Inhaltsurteile mit Hashbindungen, elf
Auswertungen je Quellfamilie, Metadatenidentitäten und376 tatsächliche
Rohdateibindungen. Referenz- und Hypothesentexte oder Ankerspans werden nicht
öffentlich wiedergegeben. Die eingefrorenen Referenzen, WAVs, Parameter und
Normalisierungsregeln bleiben unverändert.

## Tatsächliche Pflichtkriterien und Aussagegrenzen

Das clean-WER-Gate aus §13 (höchstens10% je Sprache) wird in der kanonischen
Full90-Rekonstruktion eingehalten. Die142 verbleibenden und16 zentralen
Inventaranker sind gegenüber den historischen158 Urteilen identisch: keine
verlorene Negation, falsche Zahl oder neue Segmentauslassung/-dopplung in diesen
festen clean-Ankern. Lokale Wörter-/Namensabweichungen bleiben ausgewiesen,
werden aber nicht als neue universelle Nullfehlergrenze behandelt. Der starke
masked300-Geräuschfall beschreibt eine dokumentierte Grenze und ist kein
verdeckter Ersatz für die clean-Auswertung.

Der vorhandene controlled300-Aufbau wurde unabhängig am tatsächlichen PCM
nachgerechnet:4.800.000 PCM16-Mono-Frames bei16kHz,21 unbeschnittene Quellen
lückenlos bis Frame4.599.360, danach200.640 Grundlastframes. Alle15.000
320-Frame-Fenster wurden erneut berechnet; minimaler RMS
0,0002655040258626123 liegt über Start0,00005 und Ende0,000025. WAV-, Rezeptur-
und Generierungsbindung stimmen. Damit ist die300s-/keine-sichere-Energiepause-
Bedingung der eingefrorenen Konfiguration konkret erfüllt. Alle21 Quellblöcke
sind im Text einmal erkennbar; lokale Inhaltsfehler bleiben erhalten. Eine
natürliche Monologaufnahme ist **kein zusätzliches Pflichtgate**. Das Ergebnis
belegt keine beliebige zukünftige Pausenlogik oder perfekte Worttreue.

A07/A24 bestehen für die sechs festen Empty-/Room-Eingaben dieses Laufs.
Menschliche Digitfälle sowie synthetische Ja-/Nein-/A22-Fixtures bleiben
verarbeitet und nicht pauschal weggefiltert. Dies ist keine allgemeine
Mikrofon-/Empfängerabnahme. Es wird auch kein pauschales neues Hör-Gate verlangt.

Aus dieser Qualitätsprüfung folgt kein zusätzlich verfehltes clean-WER- oder
kritisches clean-Ankergate. Eine Gesamtentscheidung zu P05 benötigt außerdem
die separaten Nachweise für Ressourcen/Ownership, Abbruch und Reproduzierbarkeit
der qualifizierten Referenz; diese werden hier weder neu als fehlend erklärt
noch allein durch Textwerte freigegeben. Die Stop-bis-Text-Grenzen gehören zu
P06 und werden nicht wegen der Zentralmediane als P05-Fehler umgedeutet.

Keine neue Hörprüfung, akustische Wortausrichtung oder behauptete innere
Decoderursache. Kalibrierrollen und synthetische Supplementfälle werden nicht
als neue blinde Abnahme ausgegeben. Kein L-, Gesamt-P05-/P06-, Mikrofon-,
Installations- oder universeller Speicherfreigabenachweis.
