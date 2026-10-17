# P05: Ganzaufnahme-VAD vor unveränderter Referenztranskription

Stand: 17.10.2026. Die zuvor gemessenen unerwünschten Leerkontrollausgaben
begründen ein positives Sprach-Gate. Das Gate verwendet die auf der getrennten
Kalibrierungsmenge geprüfte Peakvariante aus [vad-calibration.md](vad-calibration.md).
Es schneidet keine Audiodaten und verändert keine Whisper-Eingangswerte.

## Vor dem Lauf eingefrorene Konfiguration

[`reference-parameters-v4.json`](../../tests/fixtures/corpus/reference-parameters-v4.json),
SHA256 `14c36b44254d0c859074e31315b149e54b848d6601cc705ebdc1e91f683271f3`.
V4 ergänzt die lokale VAD-Fehlerkorrektur und ein warmes Silero-v5.1.2 auf CPU
mit einem Thread. Die ganze Aufnahme wird ausschließlich im getrennten Scratch
auf Peak 0,95 skaliert, Nullsignal bleibt unverändert. Erst nach vollständiger
Prüfung aller ceil(samples/512) Scores erlaubt mindestens ein Wert >= 0,5 die
bisherige vollständige Whisper-Inferenz mit Energie-Randbereich und Padding.
VAD-Fehler sind eigene Fehler und niemals leere Erfolgsantworten. Alle ASR-
Modelle, Energiegrenzen und Decoderparameter aus v3 bleiben unverändert.

Das eingefrorene 42er-Kalibrierungsmanifest bleibt
`514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49`.
Je Modell ein warmer Dienst und frische ASR-States; Auftragsfolge seriell.
Metadatenrevision `97376d6+vad-gate-v4-41884c7a` bezeichnet den Arbeitsstand,
nicht einen bereits existierenden Integrationscommit. Engine-, Gate-, Model-,
Setup- und Probequellhash stehen in der Parameterdatei. Die separate
Abnahmemenge wurde nicht transkribiert.

Zwei beschreibende Metadatenfehler bleiben im historischen Freeze bewusst
unverändert, damit sein vor dem Lauf festgehaltener Hash erhalten bleibt:
`changeFromV2` wurde aus v3 übernommen und behauptet dort noch ausschließlich
die damalige SOT-Korrektur. Für v4 gelten stattdessen `changeFromV3`, die
expliziten SpeechGate-Parameter und die gehashten tatsächlichen Quellen.
`preprocessing.mode` nennt VAD vor der Energieanalyse. Tatsächlich prüft der
Code zuerst endliche/begrenzte Samples und den Energiebereich; ohne Bereich
kommt sofort ein leeres Ergebnis. Nur energiehaltige Aufnahmen gehen komplett
durch VAD und anschließend über den schon bestimmten unveränderten ASR-Bereich.
Diese Erläuterung korrigiert keine operativen Parameter und behauptet keinen
nachträglich geänderten Freeze als vorab festgeschrieben.
Das geerbte `state.oneWarmModelContext` bezeichnet weiterhin den ASR-Kontext;
zusätzlich besitzt das separat beschriebene VAD seinen eigenen warmen Kontext.

## Tatsächliche Ergebnisse

| Modell | DE sauber | EN sauber | DE leise | EN leise | unerwünschte Ausgaben / 6 Leerkontrollen |
|---|---:|---:|---:|---:|---:|
| large-v3-turbo-q5_0 | 8/218 (3.67 %) | 5/207 (2.42 %) | 3/121 (2.48 %) | 2/114 (1.75 %) | 0/6 |
| small-q5_1 | 18/218 (8.26 %) | 9/207 (4.35 %) | 9/121 (7.44 %) | 3/114 (2.63 %) | 0/6 |

Beide Modelle liefern auf allen 36 nichtleer referenzierten Fällen byteidentische
Texte gegenüber v3, einschließlich der zehn leisen, vier kurzen und beiden
Satz-plus-Stille-Fälle. Kein Gate-bedingter Wortverlust in dieser Kalibrierung.
Satz plus Stille bleibt bei beiden Modellen bei DE 0/26 und EN 2/21 Fehlern.
Die sechs leer referenzierten Kontrollen bleiben bei beiden Modellen leer;
auch Satzzeichen würden als unerwünschte Ausgabe zählen. Lade-, Service- und
Cleanupfehler fehlen in beiden tatsächlichen 42er-Läufen.

Das ist die bestandene Kalibrierung des Gates, **keine P05-Qualifikation**.
Der zusätzliche VAD-Aufwand ist im [Ressourcenversuch](vad-resource.md) abgegrenzt.
Vollständiger unveränderter Abnahmekorpus mit Inhaltsankern und Hörprüfung,
natürlicher Fünf-Minuten-Fall, Gesamtressourcen, Abbruch und weitere native
Abnahmen bleiben offen. Dateibackendzeiten sind kein Echtzeit-L. Englisch
bleibt bis zu seiner Abnahme gesperrt.

## Modellbereitstellung und Wiederanlauf

Die Einrichtung behandelt feste Assets [gewähltes ASR, Silero] seriell.
Gesamtgröße der Referenzeinrichtung: 574.926.293 Bytes. Verifizierte Cachedateien
zählen als tatsächlich vorhanden; laufende Downloads liefern gebundene
monotone Bytefortschritte. Fehlender oder defekter VAD-Bestand verhindert
Ready; es gibt keinen Fallback ohne Gate. Die bestehende ASR-Identität im IPC
bleibt unverändert. Beide Herausgeberquellen werden vor Download angezeigt.

Echte lokale CLI-Prüfungen im isolierten ignorierten Cache:

- `download-vad` nutzte den Produkt-HTTPS-/Redirect-/Hash-/Atomic-Pfad und
  lieferte exakt 885.098 Bytes mit dem festen VAD-SHA. Keine neue Domainfreigabe.
- Ein neuer Cache enthielt nur einen Hardlink des öffentlichen ASR-Modells.
  `setup-check` endete erwartungsgemäß bei Missing mit 574.041.195/574.926.293 Bytes;
  es wurde kein Download angefordert.
- `setup-download` ergänzte VAD über den echten Downloader und wechselte
  erst nach gemeinsamer Initialisierung über Loading zu Ready mit vollständiger
  Bytezahl. Der warme Worker wurde anschließend geordnet beendet.
- `setup-check` auf beiden vorhandenen Dateien erreichte mit download=false
  erneut Ready und beendete den Worker. Das ist der lokale Cache-/Offline-Pfad;
  keine behauptete vollständige installierte Offline-Abnahme.

## Ausgeführte Prüfungen

Acht Gate-/Admission-/Snapshottests, elf Setuptests und 29 Modelltests
bestehen, einschließlich der zuvor roten neuen Verhaltensgrenzen. Der gesamte
Check und npm test sind grün. Die 25 bestehenden AppView-Tests bestehen nach
Ergänzung der zweiten Quelle; der einmalige Impeccable-Detektor meldet keine
Befunde. Das ist keine sichtbare Wry-/A23-/VoiceOver-Abnahme.

## Lokale Messartefakte

Private Texte bleiben unter `artifacts/corpus`; eingecheckt werden nur Counts
und Provenienz. Ergebnisdateien/Metadaten und Report sind separat gehasht.

| Datei | SHA256 |
|---|---|
| calibration-reference-v4-results.json | `5fc2de718bebdb12ae7c073c68d55be9a94a72a8efc73c56254357e96243951d` |
| calibration-reference-v4-results.json.metadata.json | `59f3ee1d7ec6139c05897bbcad658e5f19ccc90d546caffe1b173dc803a4010d` |
| calibration-reference-v4-report.json | `1d55298ccf8b6051cad6f743cc5ce01f390f61e647b96685775f47d6edc48ce4` |
| calibration-small-v4-results.json | `601409dd3a30358757618dbe477a11780b809d6566f0f7dd965b78fce25b3b8c` |
| calibration-small-v4-results.json.metadata.json | `dd3d813ba19d275c1c36e5607dd4fa6c850391c5246298a3d59c761ab540d09c` |
| calibration-small-v4-report.json | `70c8a497734701d2f0bfeecffbc517e56d35976c609fa2052207a7286ab0c52d` |
