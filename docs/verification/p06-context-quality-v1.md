# P06 – reduzierter Audiokontext: Dateiqualitätsvergleich

Stand: 18.10.2026. **Die isolierte Variante wird verworfen.** Beide vorab
festgelegten Vergleiche verfehlen die Qualitätsgrenzen. Die Produktionsreferenz
bleibt aktiv. P06 ist damit noch nicht abgeschlossen.

## Ausführung und Bindung

- Source: `688303557f483b6145bf2b4ff871b47537afda3d`.
- Vorab committierter [Runfreeze](../../tests/fixtures/corpus/p06-context-quality-run-v1.json):
  Commit `653715a`, SHA
  `1d09ed37d089d96275601b7eef54b8737d8b60f1f5b56178476902943cb44ba0`.
- Release-Entwicklerprobe: `corpus_probe`, measurement, Binary-SHA
  `83223aab3162b4f9127d29d5334cf1771040239e6d70ecc2c33d2a7cba2ddfe8`.
  Kein App-/DMG-Build.
- Reference → Reduced → Reduced → Reference, je ein neuer Prozess und dieselben
  unveränderten 100 Fälle. Die Paare 1/2 und 4/3 werden einzeln beurteilt.
- 02:28:10–02:35:50 UTC, 18.10.2026. Apple M5, 32 GiB, macOS 27.0.1/26A434,
  Netzstrom. Keine konkurrierenden Builds, Tests, Modelle oder Agentenarbeit.
- Ursprünglicher Prozesshandle `62986` terminal Exit0. Alle vier Prozesse und
  ihre Besitzer beendet; eigene Idle-Sleep-Assertion beendet.

38 Quellen sind gegen die unveränderliche Source gebunden, dazu Binary, beide
vollständigen Parameter, Modell/VAD, Manifest/Auswahl und sämtliche Inputs.
Die private Ausführung unter `artifacts/p06-context-quality/v1/` bleibt erhalten:
Verzeichnis0700, Rohdateien0600. Der [öffentliche Zahlen- und Belegbericht](p06-context-quality-v1.json)
enthält Hashbindungen, alle400 Messrecords und Qualitätszahlen ohne Hypothesentexte.

## Ergebnis

Die ursprünglichen90 Fälle behalten ihre festen WER-Nenner. Die zehn
Supplement-/Kalibrier-/Langkontrollen werden getrennt ausgewertet.
Beide Referenzblöcke sind bytegleich; beide Kandidatenblöcke ebenfalls.
Deshalb ergeben **beide Paare jeweils** dieselben Werte:

| Gruppe | Referenz | Reduced | Veränderung | Entscheidung |
|---|---:|---:|---:|---|
| DE clean | 46/915 = 5,0273 % | 105/915 = 11,4754 % | +6,4481 Prozentpunkte | Delta und absolute10%-Grenze verfehlt |
| EN clean | 34/734 = 4,6322 % | 54/734 = 7,3569 % | +2,7248 Prozentpunkte | Delta verfehlt |
| DE challenging | 230/559 = 41,1449 % | unverändert | 0 | Delta eingehalten; bekannte Geräuschgrenze |
| EN challenging | 5/99 = 5,0505 % | unverändert | 0 | Delta eingehalten |

Der exakte Deltatest lautet je Gruppe `200 * (E_candidate - E_reference) <= N`.
DE clean ergibt11800>915, EN clean4000>734. Weder Ausmitteln der zwei Paare
noch Hinzunehmen der Zusatzfälle verändert diese Entscheidung.

## Tatsächliche Kontextwahl und Textprüfung

Alle400 Jobs haben gültige Session-/Generation-/Job-/Bereichs-/Sprachbindungen,
vollständige Receipts und fehlerfreies Aufräumen. Jeder Referenzblock führt
94 tatsächliche Decodes mit Parameter0/effektiv1500 aus. Jeder Reduced-Block
führt81 Decodes mit1024 und13 mit0/1500 aus; bei beiden Varianten bleiben
zwei EnergyEmpty- und vier VAD-Reject-Fälle ohne erfundenen Decode. Die
vorbereiteten PCM-Längen stimmen je Fall überein. Die13 Fallbacktexte sind
ebenfalls unverändert; lange Fälle wurden tatsächlich ausgeführt.

Alle100 neuen Referenztexte und Status sind exakt identisch mit den bereits
geprüften historischen Ausgaben. Historische Urteile werden nur unter dieser
vollständigen Identität übernommen. Der Kandidat ändert11 ganze Textpaare;
89 bleiben identisch. Root las die elf vollständigen Referenz-/Baseline-/
Kandidatenpaare einschließlich ihrer25 betroffenen Originalanker manuell.
Die identischen Wiederholungen liefern dieselben Urteile für beide Paare.

Vier Fälle zeigen neue vollständige Satz-/Teilsatzdopplungen:
`fleurs-de-test-1660`, `fleurs-de-test-1694`, `fleurs-de-test-1695` und
`fleurs-en-test-1686`. `fleurs-de-test-1673` erhält einen neuen unbelegten
Zusatzsatz. Die übrigen Änderungen betreffen Zeichensetzung, Schreibweise,
Wortgrenzen oder eine wiederhergestellte Konjunktion. Der kurze deutsche
Ziffernfall behält trotz Zeichensetzungsänderung denselben Zahlenwert; daraus
entsteht kein neuer Zahlenfehler.

Die158 ursprünglichen eindeutigen Clean-Anker behalten154 pass/3 bereits
vorhandene fail/1 bereits vorhandenes unclear. Die neuen Dopplungen sind
zusätzliche Fehler der vollständigen Fälle; ein unverändert erhaltener Name
innerhalb eines gedoppelten Satzes macht dessen Wiederholung nicht korrekt.
Über alle100 Fälle verändern sich die semantischen Urteile von80pass/19fail/
1unclear auf75pass/24fail/1unclear. Vorhandene Namens-/Wortfehler und die
starken Geräuschfehler bleiben sichtbar. Kein neues pauschales Nullfehler-Gate
für Namen, keine Audiohör- oder unabhängige Labelzertifizierung behauptet.

## Entscheidung und offene Arbeit

Die isolierte Kontextvariante scheitert bereits reproduzierbar an Qualität;
weitere Echtzeitläufe sind für ihre Ablehnung nicht erforderlich. Dateizeiten
sind kein Stop-bis-Text-L und liefern keine Ressourcenqualifikation.

Die getrennte Vorabtranskription an bestätigten Pausen folgt. Soll später eine
Kombination vorgesehen werden, benötigt sie ihren eigenen vollständigen
Vergleich; die verfehlte Einzelvariante sperrt diesen Versuch nicht. Aktivierung,
finale Modellwahl und absolute Leistungsabnahme bleiben offen. Die bestehende
Referenz verfehlt weiterhin die finale Median-Latenzgrenze.
