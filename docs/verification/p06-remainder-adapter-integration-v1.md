# P06: geprüfte Integration des Restblockadapters

Der geprüfte Entwickleradapter wurde ohne Quelländerung aus
`artifacts/p06-pretranscription-screening/p06-remainder-adapter-74ffe2d.tar.gz`
übernommen, SHA256
`6ffd9799dd872aa325d11a4c3f1389fed19501123f27b627c9369957b3a3fda8`.
Alle drei vorherigen Dateien entsprachen dem unveränderlichen `74ffe2d`
bytegenau. Der archivierte Patch bestand vor der Übernahme `git apply --check`.

Nur `scripts/run-paced-remainder.py`, zugehörige Tests und das Freeze-Schema
änderten sich. Der v6-Pfad blieb unterstützt. Die zwei ausdrücklichen
Vorabtranskriptionsmodi verlangen alle elf Zeilen, vollständige Trace-/Eingabe-/
Annahme-/Abschlussbelege, monotone sitzungsübergreifende Auftragsidentitäten
und unveränderten Trace nach Join. Der gemeinsame Screening-Validator gehört
in die unveränderlichen Quellbindungen. Decoder, Modell, Runtime,
Produktstandard, Parameterprüfung und Korpus wurden nicht geändert.

## Prüfung

Unter macOS meldete `/usr/bin/python3` Python 3.9.6 und Unicode 13.0.0.

- `/usr/bin/python3 -m unittest discover -s tests/quality -p test_paced_remainder.py`:
  **21 Tests bestanden** (0.382s), einschließlich eigener Prozessgruppe mit
  `/usr/bin/time -l`: Beendigung der eigenen und Erhalt einer Kontrollgruppe.
- `node scripts/guardrails.mjs`, `node scripts/check-contracts.mjs`,
  `node scripts/secrets.mjs`: Exit 0, kein Secret-Fund.
- `git diff --check`: Exit 0.

Tatsächliche Remote-RED/GREEN-Belege und unabhängiger Review lagen vorher vor.
Geprüfter Skript-SHA256:
`38bdc39ba719ffbf53c37e9585224ddb70f6faa8a325089ab1a12d5bf8fcef51`;
Test-SHA256: `6c1021f345dfe8d4c67d01797ed385a79a37fff04aad19a32c044fa0e73b40d6`.
Keine Anpassung für Python 3.9 war nötig.

Das sind Struktur- und Prozesssicherheitsprüfungen, keine Inferenz- oder
Leistungsmessung. Die damalige Qualitätsablehnung des Kandidaten bleibt bestehen.
Kein Modell, Audio, Hypothesentext, Freeze oder Zeitarchiv wurde erzeugt/geändert;
kein P05-/P06-/nativer Produkt- oder Installationsnachweis wird abgeleitet.
