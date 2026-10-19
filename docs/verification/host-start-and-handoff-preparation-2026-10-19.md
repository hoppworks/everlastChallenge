# Vorbereitung der Start- und Übergabemessung, 19. Oktober 2026

Die Messzugänge und ihre kontrollierten Prüfungen sind vorbereitet. **Die echte 181-Start-Kampagne und eine echte Übergabe-Zeitmessung wurden noch nicht ausgeführt.** Es gibt deshalb weder gemessene Startmediane noch einen Nachweis der Grenze von 250 ms. Für diese Vorbereitung wurden keine Mikrofonaufnahme, UI-Bedienung oder physische Tasteneingabe ausgelöst.

## Startgrenze und Besitz

Der ausschließlich mit `measurement` und `test-control` verfügbare Befehl `start-host` durchläuft denselben Desktop-Dispatch, Admission-Pfad und Actor-Start wie der produktive Hotkey. Der Actor erhält `delivery=true` und `toggle=true`; sein eigenes ACK liefert die tatsächliche Session-ID. Eigener vorderster Testempfänger, positiv editierbares Feld, Rechte, Bereitschaft, unverändertes Systemstandard-Mikrofon und sauberer Idle-Zustand sind Voraussetzungen. Direkt vor dem Queue-Handoff wird unter der Admission-Sperre erneut geprüft, damit ein konkurrierender Start die Messanforderung nicht in einen Toggle-Stopp verwandelt.

Gemessen werden soll eine **obere Grenze vom kontrollierten Host-Eintritt bis zur Beobachtung des bestätigten nativen Recording-Zustands**. Vorprüfungen, Admission und Beobachtungsverzug sind darin enthalten. Nicht enthalten ist eine gemessene physische Tastatur-/OS-Zustelllatenz; der Endpunkt ist kein exakter CPAL-Callback-Zeitstempel. Der lesende Befehl `host-observation` erlaubt, den abgeschlossenen Datensatz vor Cancel abzuwarten. Fehlende, überlappende oder nicht passende Beobachtungen bleiben Fehler.

Die importierbare Vorbereitung in `scripts/host-start-campaign.mjs` plant genau einen Warmup und 30 Starts je sauberem deutschen/englischen 5-/10-/15-Sekunden-Referenzfall. Die sechs Audiohashes und der Manifesthash stehen im [JSON-Nachweis](host-start-and-handoff-preparation-2026-10-19.json). Diese Falldauern und Sprachen sind gebundene **künftige Eingabemetadaten**: Die Startkampagne spielt keine Dateien ab, führt keine ASR aus und verändert weder Sprache noch Gerät. Sie bricht nach dem bestätigten Start ab. Ein tatsächlicher Aufrufer muss einen Berichtsschreiber und den vorbereiteten Entwicklerprozess bereitstellen; Importieren startet nichts.

## Korrigierte Reviewbefunde

Die unabhängige Nachprüfung durch `recipient_metadata_review` hat beide Kampagnenbefunde als behoben bestätigt:

- Auch ein ohne Ausnahme zurückgegebenes `cleanupConfirmed=false` erzwingt nun `passed=false`.
- Eine vom Endpunkt sichtbare kontrollierte Session-ID allein ist keine Kampagnenvollmacht. Erst ein positives Start-ACK, der exakt erwartete erfolgreiche Beobachtungsordinal und dessen zum Recording-Zustand passende Session-ID frieren den Besitzer ein. Cleanup darf ausschließlich diese ID verwenden, solange aktuelle Session und kontrollierter Besitzer noch übereinstimmen. Nach Ablehnung, unbekanntem Ausgang oder fehlendem Zusammenhang werden keine fremden späteren Sitzungen übernommen oder abgebrochen.

Bei unbekanntem Startausgang bleibt Cleanup ausdrücklich unbestätigt, sofern Idle nicht bestätigt ist. Die bestehende native 60-Sekunden-Grenze bleibt zuständig; eine sichere Wiedergewinnung unbekannter Requests wird nicht behauptet. Die Nachprüfung führte selbst keine reale Kampagne aus.

## Vorhandene Prüfbelege

Die tatsächlich beobachteten fokussierten Prüfungen bestanden mit **1 Actor-, 5 Desktop- und 9 Transporttests**, **10 Kampagnentests** sowie **11 Node-Clienttests**. Die Clienttests liefen innerhalb einer früheren kombinierten Prüfung; nach den Reviewkorrekturen liefen die zehn Kampagnentests separat erfolgreich. Striktes Clippy für beide Features, normale Compile-Prüfung, Compile-Prüfung nur mit `test-control`, Oxlint, Formatierung und Diff-Prüfung bestanden ebenfalls.

Persistierte Rustbelege stehen in [`0.log`](../../artifacts/autonomous-tests/1792428092614-fa88aa85-3759-4e8b-9da8-e5cb844d34ee/0.log). [`2.log`](../../artifacts/autonomous-tests/1792428092614-fa88aa85-3759-4e8b-9da8-e5cb844d34ee/2.log) enthält auch die aktuellen zehn Kampagnen- und elf Clienttests; die Kampagnenfälle sind dort Subtests 19–28. Diese Tests verwenden kontrollierte Zustände, keine 181 echten Mikrofonstarts.

Die getrennte [Handoff-Vorbereitung](../../artifacts/native-handoff-timing-preparation-v1/preparation.json) dokumentiert 18 bestandene Native-Service-Tests einschließlich sechs neuer Messsammler-Tests, striktes Measurement-Clippy und normale Library-Compile-Prüfung. Ihre vorhandenen RED-/GREEN-/Regressions-/Clippy-/Compile-Logs werden mit SHA-256 im JSON referenziert. Die dort gebundenen beiden Native-Service-Quellhashes stimmen mit dem dokumentierten Stand überein.

Der Handoff-Sammler bindet produktive Textübernahme, exaktes Delivery-Token, begrenzte Versandintervalle und Clipboard-Cleanup. Er exportiert keine Texte oder Clipboarddaten. Versandzeitpunkte werden eingegrenzt; ein exakter Event-Post-Zeitpunkt oder tatsächlicher Empfang wird aus kontrollierten Fixtures nicht abgeleitet.

## Abgrenzung

Der laufende vollständige Softwarelauf war beim Dokumentieren noch nicht abgeschlossen; dessen vorläufiges `passed=false` ist kein abschließendes Gesamturteil und hier wird kein neuer Full-Pass behauptet. Für diese Dokumentation wurden **keine neuen Tests** gestartet. Der JSON-Nachweis bindet die betroffenen neuen Dateien sowie die wiederverwendete Startinstrumentierung und vorhandene Logs per SHA-256. Ein laufend aktualisierter Gesamtbericht wird bewusst nicht als unveränderlicher Beleg gehasht.

Echte Messungen erfolgen erst nach autonom vorbereiteten Voraussetzungen. Diese Vorbereitung aktualisiert weder den Release-Runtime-Pin noch ein installiertes Paket, ersetzt keine ASR-Qualifikation und verbraucht keine physische Geste. Die Aktualisierung des Runtime-Pins führt der Hauptagent getrennt aus.
