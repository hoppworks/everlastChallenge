# Historische Referenz fuer den naechsten P06-Qualitaetsvergleich

Dieses Protokoll regelt ausschliesslich die Wiederverwendung bereits gueltiger
**Referenztexte**. Es erteilt keine Freigabe fuer einen neuen Kandidaten und
ersetzt keine Leistungs-, Ressourcen- oder Endabnahme.

Die vollstaendige FullPrefix-Qualitaetspruefung auf Quellenstand
`301fee4cea4d9c87023bf949838d3468ecba2e95` bleibt ihre eigene, unveraenderte
historische Ausfuehrung. Quelle, Binary, Parameter, Korpus, Prozessbelege und
Urteile werden nicht auf einen spaeteren Stand umetikettiert. Der neue Vergleich
ist ausdruecklich **kein neu ausgefuehrter ABBA-Lauf**.

## Voraussetzungen

Vor dem neuen Lauf sind Kandidatenquelle, Binary, Modell/VAD, Parameter,
Zeitplan, Ein- und Ausgaben sowie Auswerter zu binden. Die Kontext-Policy muss
auch lange Eingaben eindeutig behandeln. Der bisherige kurze
1400-Kalibrierarm allein ist kein vollstaendiger Qualitaetskandidat.

Ein unabhaengiger Quellenvergleich muss bestaetigen, dass der vollstaendige
Reference0-Pfad denselben Algorithmus, dieselben vorbereiteten PCM-Samples,
Sprach- und Decoderparameter, VAD-/Modellgewichte sowie nativen Buildoptionen
beibehaelt. Die Quellen des neuen Kandidaten duerfen dabei abweichen; eine
alte 47-Quellen-Identitaet wird nicht behauptet. Ist diese Aequivalenz unklar
oder der Referenzalgorithmus veraendert, muss die betroffene Referenz frisch
laufen. Die acht Kalibriertexte allein beweisen diese Aequivalenz nicht.

## Umfang und Herkunft

- Nur die echten Referenzbloecke B1 und B4 der alten Qualitaetsausfuehrung
  werden verwendet, jeweils mit ihren getrennten Warm-/Messausgaben. Alte
  Kandidatenergebnisse werden nicht als Referenz neu bezeichnet.
- Originalbindungen fuer Freeze, Completion, Referenzparameter, Rohtexte,
  100 Fall-IDs, Manifest, Referenz-/Audiohashes und Unicode-Version bleiben
  erhalten. Vorliegende unveraenderte Prozess- und Pipelinepruefungen koennen
  ueber ihre gebundenen Abschlussbelege wiederverwendet werden.
- Der neue Kandidat laeuft in zwei vollstaendigen Bloecken ueber dieselben
  100 Faelle. Pro Fall bleiben ein Warm- und ein Messlauf erhalten: insgesamt
  200 neue Fallausfuehrungen beziehungsweise 400 neue Ergebniszeilen.
- Fehlende, fehlgeschlagene oder unerwartet leere Ausgaben bleiben sichtbar
  und sperren die Freigabe. Kein Aussortieren, kein Austausch schwieriger
  Faelle und kein Nachstimmen an Abnahmetexten.

## Unveraenderte Qualitaetsgrenzen

Die vorhandene kanonische Auswertung und Unicode-Version 13.0.0 gelten
weiter. Fuer jede der beiden Wiederholungen werden Warm- und Messphase
getrennt gegen die historischen Referenzen verglichen. Die Original90 werden
je Sprache und Klasse mikroaggregiert, die zehn Zusatzfaelle separat
ausgewiesen. Wiederholungen und Zusatzfaelle veraendern keinen
Referenzwortnenner der Original90. Je Sprache und Klasse bleiben hoechstens 0,5
Prozentpunkte Verschlechterung und fuer saubere Sprache hoechstens 10 % WER
zulaessig. Die urspruenglichen Stille-, Kurzsprach-, Anker-, Rand-, Lang- und
Kontrollfaelle bleiben enthalten.

Alle neuen Texte werden gegen die gebundenen Originalreferenzen geprueft.
Bytegleiche Texte koennen die bereits belegten Inhaltsurteile uebernehmen;
abweichende Texte benoetigen die volle betroffene Inhalts-/Ankerpruefung.
In den festen Anker-/Segmentfaellen duerfen keine neuen verlorenen
Negationen, falschen Zahlen oder Segmentauslassungen/-dopplungen auftreten.
Vorhandene ASR-Defekte bleiben als solche sichtbar und werden nicht zu
korrekten Ausgaben umgedeutet. Zahlen-, Namens- und Negationsfehler werden
nicht durch Normalisierung oder alte Urteile verdeckt.

## Getrennte Validierung

Die vorhandenen Fresh400-/47-Quellen-Validatoren bleiben streng. Eine separate
Vergleichsprojektion benennt historische und neue Authorities, Wiederholungen,
Quellenunterschiede und den unabhaengigen Aequivalenzbeleg explizit. Sie darf
weder frische Completionobjekte erfinden noch Quellenpruefungen ueberspringen.
Die neue Freigabe setzt zusaetzlich tatsaechliche neue Pipeline-/Request-/
Kontext-/PCM- und Abschlussbelege voraus.

Aus dieser Textwiederverwendung folgen keine kalten oder warmen Startzeiten,
kein Stop-bis-Text-L und keine Ressourcenwerte. Nach bestandener Qualitaet
benoetigt der gewaehlte neue Kandidat weiterhin frische Referenz/Kandidat-
Leistungspaare in beiden Reihenfolgen, die vorgeschriebenen Wiederholungen
und absoluten Grenzwerte, anschliessend Restkorpus und Ressourcenabnahme.
