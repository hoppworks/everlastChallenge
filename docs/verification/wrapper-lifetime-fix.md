# Rust-VAD: Besitz des Modellpfads

Stand: 17.10.2026. Kandidat auf `cd47ff5`, Wrapper-Modul SHA256
`a4e481fac0b423ab532e3f4b8e3348a23e3134a10e4581ea3f383d6a81e6438d`.

## Ursache und Änderung

Der bisherige whisper-rs-0.16.0-Konstruktor übergab den Modellpfad mit
CString::into_raw, ohne die Allokation später freizugeben. Der lokale Fork
behält die CString während des synchronen nativen Ladeaufrufs und übergibt
as_ptr. Danach wird sie auf Erfolgs- und Fehlerpfad automatisch freigegeben.
Der gepinnte C++-Loader liest synchron und kopiert den Pfad in std::string.

Die vollständige veröffentlichte Paketbasis bleibt erhalten; nur das
Konstruktormodul ist verändert. Herkunft, Inventar, Quellhashes und genaue
Regression stehen in [PATCHES.md](../../vendor/whisper-rs/PATCHES.md).
Die Cargo-Patchtabelle wählt den lokalen Wrapper. Im Root-Lockfile entfallen
nur dessen Registryquelle und -checksum; Versionen und Abhängigkeitskanten
bleiben gleich. Offline-Cargo-Metadaten bestätigen den lokalen Pfad und drei
unveränderte Workspace-Mitglieder.

## Ausgeführte Prüfung

Die native Testfixture kompiliert das gesamte tatsächliche Wrapper-Modul.
Ein synchroner FFI-Stand-in liest den Pfad; ein beobachtender Systemallocator
prüft dessen Freigabe vor Rückkehr aus dem Konstruktor. Je 100 erfolgreiche
und fehlerhafte Aufrufe: Original 0/100 und 0/100 freigegeben (erwartetes RED),
Patch 100/100 und 100/100 (GREEN). Beide Varianten beobachten 200 gültige
Pfade und 100 Context-Drops. Der Stand-in dereferenziert den Fakehandle nie.
Der Offline-Launcher rekonstruiert die Originaldatei mit exakter SHA-Prüfung;
auch der separat übergebene unveränderte Registry-Originalpfad wurde geprüft.
Der Offline-Fall ist Teil der regulären Python-Qualitätssuite.

Folgende integrierte Befehle liefen erfolgreich:

- `npm run check`
- `npm test` einschließlich jetzt 42 Python-Qualitätstests
- `cargo deny check`
- `node scripts/secrets.mjs` ohne Fund
- `cargo build --release --locked --example model_probe --example transcribe_probe --example corpus_probe --example vad_resource_probe`

Die echten nativen Werkzeuge wurden gegen den lokalen Wrapper gebaut:

- `model_probe setup-check artifacts/models`: beide Modelle geprüft und geladen,
  Ready mit 574.926.293 verifizierten Bytes.
- `transcribe_probe` mit der autorisierten privaten Aufnahme: drei Wiederholungen,
  jeweils 56 Bytes und identisch zur früheren Modellbaseline; rund 977/962/961 ms.
  Das belegt Wiederholbarkeit, keine unabhängig bestimmte Erkennungsgenauigkeit.
  Audiodaten und Wortlaut bleiben außerhalb von Git.
- `vad_resource_probe` mit konstruierten 300 s und 100 weiteren Kurzaufrufen:
  9.375 Wahrscheinlichkeitsfenster, maximale Wahrscheinlichkeit
  0,04771236330270767, Original/Scratch je 19.200.000 Bytes angeforderte Kapazität.
  Diese beobachteten Werte stimmen exakt mit dem Vorversuch überein.

Lokales Ergebnis `artifacts/corpus/vad-resource-wrapper-fixed-max300.json`,
SHA256 `a877da2a106d5e0a71ff6ad4e58d3ac74860aa58c03a4a681e5df38c1f8e8baf`,
Quellkennung `cd47ff5+wrapper-cstring-a4e481fa`.
Der native C++-Quellstand bleibt
`fc7f40c5d5022f45920164bfe8261de110d98a4fb88fd1ee97ef526ab00e682e`.

## Aussagegrenze
