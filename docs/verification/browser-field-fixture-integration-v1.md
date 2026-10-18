# Lokale Integration der Browserfeld-Fixture

18.10.2026. Die zuvor unabhängig geprüfte Fixture liegt unverändert unter
tests/fixtures/browser-fields/. Herkunft ist das private Archiv
everlast-browser-field-fixture-5bbe330-v2.tar.gz, SHA256
`eff79eb50a88c328bd036e20d91c767f309b812ea89123150125f1115f88b6e8`.
Alle acht Mitglieder sind reguläre Dateien unter dem erwarteten Präfix; die
sieben Nutzdateien stimmen in Größe und Hash mit PROVENANCE.json überein.

Auf dem Mac bestehen sechs Node-Tests des tatsächlichen Inline-JavaScripts
mit kontrollierten DOM-/Timer-Doubles sowie fünf Python-Tests der statischen
Seite und negativen Vertragsänderungen. Befehle im Fixtureverzeichnis:

```
node --test fixture.test.cjs
python3 -m unittest -v test_validate.py
```

Die öffentliche Seite enthält 18 Feld-/Kontextfälle; alle 20 Workflows der
originalen Matrix bleiben `not-run`, Beobachtungen `null`. Sie enthält keine
Appanbindung, Eingabewert-/Passwort-/Clipboard-Lektüre oder Netzwerkzugriffe.
5000 ms ist die nominelle Timerverzögerung; der tatsächlich ausgeführte Zustand
ist für spätere Tests abzuwarten. Dummy-Passwortfeld und synthetische ARIA-Liste
beweisen keine globale Secure-Input-Aktivierung oder echte Autofill-Interna.

Kein Browser wurde für diese Integration gestartet, keine native AX-/TCC-/
Hotkey-/Transport-/Empfängerabnahme durchgeführt. Die unveränderte öffentliche
68-Byte-Transportfixture besitzt SHA256
`78ce796361f50b77b1c86a693c9ba3bb02be2be8dc7fcd0e5ddfd4305da0c5b7`.
Tatsächliche Einfügebeobachtung und Mikrofon-/Inferenz-E2E bleiben getrennte
Pflichten; P02K/P07 werden mit dieser Vorbereitung nicht abgeschlossen.
