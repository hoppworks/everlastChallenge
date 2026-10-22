# Gatekeeper-Nachprüfung des Downloads

Untersucht wurde die entpackte Version 0.2.0 aus dem veröffentlichten ZIP mit
SHA-256 `5ad370e37cf71b9a36803c8b309fc5fa21076569d246ac22c96e8f1aeda45fbe`.

| Prüfung | Ergebnis |
|---|---|
| `codesign --verify --deep --strict APP` | Exit 0: Signaturintegrität erhalten |
| Signaturmetadaten | lokale Entwicklungsidentität, keine Apple-Team-ID |
| `spctl --assess --type execute APP` | Exit 3: rejected |
| `xcrun stapler validate APP` | Exit 65: kein angeheftetes Notarisierungsticket |

Die beim Öffnen sichtbare Meldung passt zur Gatekeeper-Ablehnung. Eine
unbeschädigte lokale Signatur ersetzt keine Apple-Developer-ID-Signierung und
Notarisierung. Die vorherige Bundleprüfung untersuchte Integrität und
Mikrofondeklaration, nicht die Zulassung eines heruntergeladenen Programms.

Der neue Befehl `npm run verify:distribution -- APP` prüft zusätzlich Gatekeeper
und das Notarisierungsticket. Regressionstests verhindern, dass allein ein
erfolgreicher Signaturcheck als bestandene Vertriebsprüfung gilt. Gegen das
unveränderte veröffentlichte Paket bleibt dieser Befehl korrekterweise rot.
Eine erfolgreiche Notarisierung oder manuelle Freigabe wird nicht behauptet.

Für den bestehenden lokalen Build ist die
[von Apple dokumentierte manuelle Freigabe](../usage.md#macos-blockiert-das-erste-öffnen)
erforderlich. Der reguläre Weg über Developer ID und Notarisierung steht in
der [Entwicklungsanleitung](../development.md#gatekeeper-und-notarisierung).
Das ZIP und seine Prüfsumme wurden bei dieser Diagnose nicht verändert.
