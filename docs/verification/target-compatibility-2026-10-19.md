## Verhalten

Die Zielerkennung versucht systemweiten Fokus, Fokus der gebundenen App und deren fokussiertes Fenster. Jeder Kandidat muss dieselbe PID und Prozessstartidentität besitzen. Ein fehlender Suchweg oder undurchsichtiger Container darf zum nächsten Weg führen; falsche Werttypen, fremde Prozesse, sichere oder explizit deaktivierte Textfelder dürfen nicht so übergangen werden. Nach erfolgreicher Erfassung bleibt dasselbe AX-Element gebunden.

Bei bekannten normalen Textrollen genügt eine positiv bestätigte Fähigkeit zum Ersetzen von `AXValue` oder `AXSelectedText`, solange kein explizites `AXEnabled=false` gemeldet wird. Fehlende Aktivierungsmetadaten allein blockieren damit nicht mehr. Ein schreibbarer Auswahlbereich allein bleibt unzureichend. Ohne positiven Schreibweg bleiben explizit negative und unbekannte Editierbarkeit getrennte Ergebnisse. Die reguläre Secure-Input- und Berechtigungsprüfung bleibt erhalten.

Nach dem einmaligen Versand beobachtet der AX-Owner denselben Zielkontext. Wenn ein begrenzter Vorher-/Nachherwert und eine gültige UTF-16-Auswahl verfügbar sind, prüft er die exakte erwartete Ersetzung. Fremde Änderungen werden nicht als richtige Einfügung gewertet. Nicht verfügbare Inhaltsprüfung bestätigt nur den Versand, nicht den Text. Beobachtete Abweichung oder Zielwechsel ergibt einen unbestätigten Abschluss, ohne zweiten Einfügeversuch.

Die vorherige Textaufnahme ist auf 128 Ki UTF-16-Einheiten beziehungsweise 256 KiB UTF-8 begrenzt, verbleibt im AX-Owner und wird bei der Nachprüfung verbraucht. Verfall/Abbruch räumt ungenutzte Aufnahmen auf. Kein Zielinhalt wird protokolliert oder persistiert. Die Clipboard-Frist nach Versand bleibt auch bei negativer Nachprüfung erhalten; vor dem Versand wird weiterhin sofort aufgeräumt.

## Prüfung und Grenzen

Die Klassifikation und Suchreihenfolge werden über dieselbe produktive Entscheidungsfunktion geprüft. Negative Gegenfälle umfassen explizit deaktivierte, schreibgeschützte, sichere und unbekannte Felder, falsche Typen und fremde PID. Eigene unsichtbare AppKit-Controls bestätigen den Wechsel beider Schreibfähigkeiten bei `setEditable(false)`; sie sind kein Ersatz für prozessübergreifende AX-Abnahme.

Die neuen Nachprüfungstests verwenden den echten Runtime-/NativeService-Owner. Sie prüfen exakte Einfügung, nicht prüfbaren Inhalt, beobachteten Fehler, nur einen Versand und Erhalt der Restore-Frist. Die Worker-Tests trennen den committed Beobachtungsauftrag strikt von pending-only Freigaben und fremden beziehungsweise ersetzten Sitzungen. Ein unabhängiger Reviewer identifizierte und prüfte die Korrektur von zu breitem Fallback, verkürzter Restore-Frist und einem widersprüchlichen Planabsatz.

Der Entwickler-Testkanal bietet zusätzlich `inspect`: ausschließlich Metadaten des konfigurierten eigenen Testempfängers, ohne Fokuswechsel, Audioaufnahme oder Auslieferungsberechtigung. Das ist eine Diagnose und kein positiver Einfügenachweis. Vollständige System-, Notes-, ASR- und Leistungsabnahmen bleiben getrennt.

## Ausgeführte Prüfungen

Das Vollprofil bestand mit 446 Rust-, 215 UI-, 42 Node- und 204 Python-Tests.
Die abschließende Größen-Vorprüfung vor einem optionalen `AXValue`-Abruf wurde
danach durch 19 Target-/Worker-Tests und Feature-Clippy geprüft. Eine fehlende,
negative, gebrochene oder zu große Anzahl verhindert den Inhaltsabruf; die
Antwortlänge wird anschließend erneut kontrolliert. Die Ziel-App kann zwischen
diesen Abfragen ihren Inhalt ändern; es wird keine atomare Größenbindung
behauptet. Die [Dateibindung](target-compatibility-2026-10-19.json) trennt
Vollprofil, spätere gezielte Prüfung und weiterhin fehlende OS-Abnahme.

## Tatsächliche prozessübergreifende Metadatenprüfung

Der lokal signierte Entwicklerbuild aus `212d5e7` las ausschließlich technische
Metadaten des eigenen nativen Testempfängers. AXEnabled und die optionale
Unterrolle waren mit -25205 nicht verfügbar; AXValue und AXSelectedText waren
schreibbar. Die neue Klassifikation ergab `captureEligible: true`, während die
alte zwingende Enabled-Bedingung das Feld zurückgehalten hätte. Beide Rechte
blieben erhalten. Keine Aufnahme, kein Inhaltsabruf und keine Einfügung: Diese
Beobachtung bestätigt die Zielklassifikation, nicht den vollständigen Versand.
