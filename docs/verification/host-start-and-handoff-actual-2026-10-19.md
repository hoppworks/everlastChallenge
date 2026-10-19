# Tatsächliche Start- und Übergabebelege, 19. Oktober 2026

Die vorhandenen Belege wurden unabhängig gelesen, ihre Hashbindungen geprüft und alle sechs Startmediane aus den Einzelwerten neu berechnet. **181 tatsächliche kontrollierte Starts sind abgeschlossen: ein Warmup und 180 gewertete Starts, ohne Fehler, mit bestätigtem Cleanup.** Die sechs Median-Obergrenzen liegen zwischen **116,776 und 129,944 ms**, jeweils unter 250 ms. Zusätzlich belegt eine kurze echte Sprachsitzung die native Übergabe und anschließend abgeschlossenen Clipboard-Restore.

Diese Prüfung hat keine Tests, UI-Aktionen, Aufnahmen oder Wiedergaben ausgelöst. Der [JSON-Nachweis](host-start-and-handoff-actual-2026-10-19.json) enthält die einzelnen Prüfergebnisse und SHA-256-Bindungen.

## Startkampagne

[`campaign.json`](../../artifacts/host-start-actual-v2/campaign.json) enthält exakt die Ordinale 1–181, 181 eindeutige Session-IDs, einen Warmup, keine fehlenden Zeiten oder Trial-Fehler und `passed=true`, `cleanupConfirmed=true`. Jede Gruppe enthält genau 30 gewertete Starts. Der Median ist jeweils das Mittel der beiden mittleren sortierten Einzelwerte; alle neu berechneten Werte stimmen exakt mit dem Bericht überein.

| Gebundener künftiger Eingabefall | Gewertete Starts | Median der Start-Obergrenze |
| -------------------------------- | ---------------: | --------------------------: |
| DE / 5 s                         |               30 |                  122.751 ms |
| DE / 10 s                        |               30 |                  117.403 ms |
| DE / 15 s                        |               30 |                  118.543 ms |
| EN / 5 s                         |               30 |                  117.356 ms |
| EN / 10 s                        |               30 |                  116.776 ms |
| EN / 15 s                        |               30 |                  129.944 ms |

Die sechs Sprach-/Dauerfälle sind vorgebundene **künftige Eingabemetadaten**. Ihre WAV-Dateien wurden für diese Startkampagne weder abgespielt noch transkribiert; Sprache und Gerät wurden nicht je Fall umgestellt. Tatsächlich gemessen wurde jeweils der produktive kontrollierte Host-Start mit Systemstandard-Mikrofon bis zur Beobachtung des nativen Recording-Zustands, danach Cancel. Es ist eine konservative obere Grenze einschließlich Beobachtungsverzug, kein exakter CPAL-Startzeitpunkt und kein physischer Tastatur-/OS-Zustellmedian.

Die ursprünglichen Bindungen des Berichts stimmen mit den vorhandenen Dateien überein: Empfängerquellstand einschließlich URI-Startupfix, zentraler Referenzmanifesthash und das separat erhaltene Entwickler-Bundle-Executable. Dessen SHA-256 ist `44ef3bf187a8bfc05f16ad18f70e7276389a5f355d7441eb98be678783007a9c`. Der genaue erhaltene Pfad steht im JSON. Der damals noch nicht committete Empfängerquellstand ist über seinen übereinstimmenden Dateihash gebunden. Ein anderer aktueller Debug-Build wird nicht als dieses Messartefakt ausgegeben.

Der tatsächliche [Shutdown-Export](../../artifacts/host-start-actual-v2/native-start-shutdown.json) enthält dieselben 181 Rohdatensätze: Ordinal, Session-ID, Start-Obergrenze und ACK-Zeit stimmen für jeden Fall mit der Kampagne überein. Er bestätigt den gejointen Shutdown, keinen ausstehenden Beobachtungsordinal, keinen Overflow und keinen Sammlerfehler. Sein eigenes `qualification=false` bleibt bestehen; die Auswertung der begrenzten Startregel ist die oben dokumentierte unabhängige Rechnung.

Der [Umgebungs-Readback](../../artifacts/host-start-actual-v2/environment-readback.json) wurde **nach** Kampagne und kurzer Übergabe um 17:04:35 UTC aufgenommen: macOS 27.0.1, Apple M5, 32 GiB RAM, Netzstrom, Rust/Cargo 1.93.0, Xcode 27.1 und Clang 21. Der Operator berichtet zusätzlich eine Netzstrombeobachtung beim Kampagnenstand 62. Dies ist keine erfundene Vorprüfung oder fortlaufende Energiemessung für jeden Einzelstart. Der Readback bindet Modellcachehash, Mess-Binary, Buildlog, Release-Konfiguration und Shutdown-Export. Die vier referenzierten Datei-Prüfsummen wurden bei dieser Review erneut abgeglichen; das große Modell wurde für die Review nicht erneut gelesen. Sein Modellhash bleibt als damals protokollierter Readbackwert gekennzeichnet.

## Übergabe und ursprünglicher Harnessfehler

Der ursprüngliche Bericht [`handoff.json`](../../artifacts/host-start-actual-v2/handoff.json) bleibt unverändert erhalten und meldet ausdrücklich **`passed=false`**, bei `error=null` und `cleanupConfirmed=true`. Er enthält bereits erfolgreichen committed Versand und den tatsächlichen Empfang, beobachtet den asynchronen Clipboard-Abschluss aber noch mit Nullwerten. Laut der erhaltenen Harness-Erklärung prüfte die anfängliche Zusammenfassung außerdem ein nicht vorhandenes `recordCount`-Feld. Dass dieses Feld im Beobachtungsschema fehlt, ist unabhängig sichtbar. Das ursprüngliche false-Ergebnis wird nicht nachträglich in einen Pass umgeschrieben; auch sein allgemeines Cleanup-Feld ersetzt nicht den damals noch fehlenden Clipboard-Abschluss.

Ein anschließender **rein lesender** Abruf ist in [`handoff-completed.json`](../../artifacts/host-start-actual-v2/handoff-completed.json) erhalten. Er bestätigt **dieselbe Session 182**, dieselben Versandintervalle und dasselbe Delivery-Token-Matching; `invalid`, `overflow` und `observerContentionOrPoison` sind falsch. Es wurde keine Session oder Audioaufnahme wiederholt.

- Produktiver Textbereit-Zeitpunkt bis Versand: **109,340708–120,210542 ms**; `sendSucceeded=true`, `sendCommitted=true`.
- Derselbe Textbereit-Zeitpunkt bis Clipboard-Abschluss: **627,160167–628,307125 ms**; `cleanupSucceeded=true`, `restoreOutcome=restored`.
- Der erhaltene Empfängernachweis zeigt genau einmal den vollständigen öffentlichen Testsatz: **134 UTF-8-Bytes**, Feld- und Gesamtumfang identisch, kein zusätzliches Kürzelzeichen. Der SHA-256 `161df165fab1271854e8ff49cb989e6f2f9d9192b3c02a06e7384de01534f214` stimmt unabhängig mit dem festgelegten öffentlichen Erwartungssatz überein. Das Produkt meldet `sent` und anschließend `Idle`.

Der spätere Bericht übernimmt den **bereits vorhandenen** Empfängersnapshot aus dem ersten Bericht unverändert; er behauptet keinen zweiten frischen Empfang. Die frisch nachgelesene Information ist der abgeschlossene native Handoff-/Restore-Datensatz. Versand und Clipboard-Abschluss sind getrennte Zeitintervalle ab Textbereit, keine Stop-bis-Text-Zeiten und keine exakten Event-Post-Zeitpunkte. Es handelt sich um einen tatsächlichen Übergabefall, nicht um eine Verteilungs- oder allgemeine Kompatibilitätsmessung.

## Grenzen und eigener Empfängerstart

Diese Belege betreffen den instrumentierten Entwicklerstand, kontrollierte reale Systemstandard-Mikrofonstarts und eine kurze echte Sprachübergabe an den eigenen separaten nativen Empfänger. Sie ersetzen weder die physische globale Ein-Gesten-Abnahme des normalen installierten Kandidaten noch eine neue ASR-Korpusqualifikation. Aus einem passenden öffentlichen Satz folgt keine allgemeine Erkennungsqualitäts- oder Fremdapp-Zusage.

Der eigene Empfänger erhielt seinen Launchkontext über den gewöhnlichen Browser-Link-/LaunchServices-Öffnen-Weg mit einem einzigen festen URI. Dieser war vor der Kampagne verbraucht; spätere URI-Aktivierung ist im selben Prozess damit gesperrt. Getrennte Quellgrenze: Die `preparing`-Prüfung verwendet den zuletzt beobachteten lokalen Signalzustand, erlaubt einen fehlenden Reporter und schließt einen gestoppten Reporter nicht gesondert aus. Sie garantiert keine atomare Aktualität gegenüber Signaldateiänderungen. Diese Grenze wird nicht als zusätzliche Timingmessung ausgegeben und verändert die oben geprüften verbrauchten-URI-Belege nicht.
