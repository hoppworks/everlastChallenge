# Nativer App-Vergleich vom 22.10.2026

**Das Ergebnis ist gemischt.** Beim geprüften Everlast-Kandidaten erscheint der
Text in zwei sauberen Lesefällen früher. Die vier Fälle mit Korrekturen brauchen
länger; in drei davon bleibt der Inhalt nur bei Everlast vollständig erhalten.
Beim Zitatfall verfehlen beide Apps das vollständige Bereinigungsziel.
Ein allgemeiner Geschwindigkeits- oder Qualitätsvorsprung ist damit nicht belegt.

## Konfiguration und Messgröße

Verglichen wurden Everlast aus Quellrevision
`ca56a99fdab898e53e944f309d3c328d25548667` mit aktiviertem lokalem
MLX-Textworker (Policy 8) und Voicely 0.4.27 im lokalen Privatmodus mit Tonalität
„Original“ und `large-v3-turbo`. Der Kandidat ist nicht automatisch mit dem
zuletzt veröffentlichten Download identisch. Binär- und Modellhashes stehen im
[Datenauszug](../benchmarks/native-comparison-2026-10-22.json).

Bei Voicely war zusätzlich Ollama mit `qwen2.5:7b` konfiguriert. **Zehn der zwölf
Läufe protokollieren tatsächlich `deterministic_clean_original`, `llm=none`.**
Bei zwei Läufen fehlt der entsprechende Eintrag. Aus der Konfiguration wird kein
LLM-Aufruf abgeleitet. Dies ist kein isolierter Vergleich von MLX- und
7B-Modellinferenz und kein Test anderer Tonalitäten.

Beide Apps erhielten seriell dieselben Audiodateien über die eingebauten
Lautsprecher und das Mikrofon eines Mac17,2 mit 32 GiB RAM und macOS 27.0.1.
Die Ausgabelautstärke war 100 %, der aufgezeichnete Eingangspegel etwa 28,6 %.
Die akustisch aufgenommenen PCM-Daten sind deshalb nicht identisch.
Builds und andere Benchmarks liefen während der Messungen nicht parallel.

Gemessen wird **vom auf die monotone Uhr projizierten Ende der abgespielten
PCM-Datei bis zum vollständig beobachteten finalen Text im eigenen nativen
Zielfenster**. Eine fortlaufende Empfangsbeobachtung begrenzt den Endzeitpunkt;
der Text muss anschließend 500 ms stabil bleiben. Es wird kein akustischer
Laufzeitwert abgezogen. Ein natives Stoppsignal wird ungefähr 500 ms nach dem
Dateiende angefordert; tatsächlicher Abstand und eine separate Zeit ab
Stoppsignal sind pro Lauf gespeichert.

Diese Kennzahl umfasst den verbleibenden App-Ablauf einschließlich Einfügen.
Sie misst weder das physische Ende der Stimme am Mikrofon noch reine Inferenz
oder automatische Stilleerkennung. Ein gesendetes Einfügeereignis allein reicht
nicht als Empfangsnachweis.

## Beobachtungen

24 Läufe: zwölf je App, sechs unterschiedliche deutsche Fälle. Drei Fälle
wurden vor Sichtung der Ergebnisse für jeweils drei Wiederholungen festgelegt.
Die Tabelle zeigt gerundete Mittelpunkte der erfassten Zeitintervalle; bei
mehreren Wiederholungen deren deskriptiven Median. Ein Einzellauf ist keine
stabile Schätzung.

| Fall                         | Gepaarte Läufe |  Everlast |  Voicely | Inhaltsprüfung aller Läufe dieses Falls                     |
| ---------------------------- | -------------: | --------: | -------: | ----------------------------------------------------------- |
| FLEURS de 1510, 9 s          |              1 |  1.228 ms | 1.985 ms | beide bestanden; kleinere Interpunktionsgrenze bei Everlast |
| FLEURS de 1513, 5,4 s        |              3 |  1.174 ms | 1.550 ms | beide bestanden                                             |
| Wochentag korrigieren, 4,7 s |              1 |  2.944 ms | 1.653 ms | Everlast bestanden; Referenz lässt den Satzgegenstand aus   |
| Plan, 60,2 s                 |              3 | 11.840 ms | 3.647 ms | Everlast bestanden; Referenz verliert bzw. verändert Inhalt |
| Verneinung, 9,7 s            |              1 |  3.240 ms | 1.688 ms | Everlast bestanden; Referenz lässt Satzteile aus            |
| Zitat, 8,8 s                 |        2 von 3 |  3.320 ms | 1.649 ms | beide in allen drei Läufen durchgefallen                    |

Beim Zitat wechselte der Everlast-Worker im ersten Lauf seinen Prozess. Dieser
Lauf bleibt im Datensatz und in der Inhaltsprüfung enthalten; für die Tabelle
werden **bei beiden Apps nur Wiederholungen 2 und 3** verwendet. Das ist eine
unvollständige gepaarte Teilserie, keine bestandene Drei-Läufe-Warmstartserie.
Auch die übrigen Werte werden nicht als vollständig qualifizierte Warmstarts
bezeichnet: Prozesskontinuität beweist nicht sämtliche internen Modellcaches,
und die Modellresidenz der Referenz wurde nicht direkt instrumentiert.

In den zwei beidseitig bestandenen Lesefällen liegen Everlasts beobachtete
Zeiten rund 38,1 % bzw. 24,3 % niedriger. Diese Prozentwerte gelten nur für die
jeweilige kleine Stichprobe. Schnellere, aber inhaltlich fehlerhafte Ausgaben
werden nicht als Sieg bei gleichwertiger Aufgabe gewertet.

## Inhaltsprüfung und Grenzen

Zwei KI-gestützte Lesungen prüften die Ergebnisse gegen die festgelegten
Inhalte: Namen, Zahlen, Verneinungen, Korrekturen und Vollständigkeit. Insgesamt
bestehen bei Everlast 9 von 12 Ausgaben, bei Voicely 4 von 12. Wiederholungen
zählen dabei mehrfach; es sind **keine zwölf unabhängigen Sprachbeispiele**.
Auf Ebene der sechs unterschiedlichen Fälle bestehen fünf bzw. zwei. Dies sind
weder eine allgemeine Genauigkeitsquote noch eine menschliche Freigabe.

Im Zitat bleibt bei Everlast ein ungewolltes anfängliches „Im“ erhalten.
Die Originalansicht der zweiten Wiederholung enthält es bereits vor der
Bereinigung. Das belegt die Herkunft für diese Wiederholung, nicht pauschal für
alle Fehler. Die Referenz verliert zusätzlich den vollständigen Satz zur
Nachricht. Aus diesem Fall wird keine Siegerdarstellung abgeleitet.

Weitere Grenzen:

- Ein Mac, sechs deutsche Fälle, gelesene Sprache und synthetisch gesprochene
  Korrekturbeispiele. Kein allgemeiner Test spontaner Sprache, anderer Räume,
  Akzente oder Hardware. Die Fälle sind kein unangetastetes Blindtestset.
- Unterschiedliche Stille vor dem abgespielten Text: etwa 6,36–9,69 s bei
  Everlast und 3,34–27,99 s bei der Referenz. Startbestätigungen und technische
  Vorbereitungsverzögerungen sind erhalten, nicht nachträglich angeglichen.
- Eine anfängliche Timerparametrisierung und zweimal verzögert sichtbare
  Startprotokolle wurden vor der jeweiligen Wiedergabe behandelt. Die schon
  laufende Aufnahme wurde fortgesetzt; zusätzliche Vorlaufstille bleibt Teil
  des jeweiligen Ablaufs. Keine stillschweigend ersetzten Ergebnisläufe.
- Die Referenz-Rückmeldetöne blieben aktiviert. Nach manchen abgeschlossenen
  Läufen wurde eine hängen gebliebene Command-Taste im eigenen Testfenster
  gezielt freigegeben. Das geschah nach gesichertem Empfang, außerhalb der
  Zeitmessung; die Aktionen sind separat protokolliert.
- Englisch wurde nicht ausgeführt, weil der geprüfte Everlast-Produktpfad nur
  Deutsch anbietet. Eine getrennte Kaltstartserie wurde nicht ausgeführt.
- Es werden keine p95-, Signifikanz-, Energie- oder Speicherbehauptungen und
  keine Garantie „immer gleich schnell oder schneller“ abgeleitet.

## Daten nachvollziehen

Der [numerische Auszug](../benchmarks/native-comparison-2026-10-22.json) enthält
alle 24 Beobachtungen, Zeitgrenzen, Qualitätsurteile, die ausgeschlossene
Wiederholung und die verfügbaren Pfadbelege. Für einen Fall ergibt sich ein
Zeitmittelpunkt in Millisekunden aus
`(latencyEarliestNanos + latencyLatestNanos) / 2_000_000`; mehrere Werte werden
sortiert und ihr Median berechnet. Die Zeilenauswahl steht in
`commonRepetitions`. Bei der Zitat-Teilserie wird kein vollständiges
Drei-Läufe-Ergebnis behauptet.

Der Auszug enthält keine persönlichen Pfade, Prozess-IDs, Rohaufnahmen oder
vollständigen Transkripte. Hashes binden die lokal aufbewahrten Originalbelege,
ersetzen aber keine öffentliche Einsicht in diese Belege. Nachrechenbar sind die
veröffentlichten Zahlen; die vollständige Aufnahme, Semantik und damalige
Laufzeit lassen sich aus diesem Auszug allein nicht unabhängig reproduzieren.
