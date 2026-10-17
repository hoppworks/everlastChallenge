# P05: vorab festgelegte Auswahl vollständiger 30-s-Diktate

Stand: 17.10.2026. Auswahlregeln vor Sichtung der Kandidatenliste festgelegt:

1. Ausschließlich vorhandene, auf FLEURS-Revision `70bb2e84b976b7e960aa89f1c648e09c59f894dd` gepinnte DE/EN-Originalquellen. Keine ASR, Downloads, Bauprozesse oder Hypothesenprüfung.
2. Je Sprache zuerst `test`, nur bei fehlendem Kandidaten `dev`. Vollständiger Quellclip mit **464.000 bis einschließlich 480.000 Frames** bei 16 kHz, also 29 bis 30 Sekunden. Keine Verkettung, kein Schneiden und keine Auswahl nach erkanntem Text.
3. Nach Dauerfilter aufsteigend nach ganzzahliger `sentenceId`, dann Dateiname sortieren; erster Kandidat je Sprache. Weitere Kandidaten dienen nur der dokumentierten Bestandsübersicht. Der jeweilige Originaltext aus TSV-Spalte 3 (nullbasierter Index 2) bleibt die unabhängige Referenz.
4. Originalclip vollständig Float32→PCM16 mit dem vorhandenen geprüften Builder konvertieren; ausschließlich am Ende bis genau 480.000 Frames mit Nullframes ergänzen. Maximum 16.000 Nullframes, keine zusätzliche Lautstärke-/Pausenänderung.
5. Vorherige Verwendung derselben Quelle/Satz-ID in eingefrorenen Fällen und im konstruierten Langfall ausdrücklich erfassen. Neue Clip-ID ist kein neuer unabhängiger Satz und keine nachträgliche Erweiterung des eingefrorenen 90er-WER-Gates.
6. Nur bei einem passenden Fall je Sprache ein getrenntes Zweifallmanifest veröffentlichen. Vor jeder Veröffentlichung Archiv-/TSV-Pins und tatsächliche WAV-Frames/Hashes prüfen; bestehende Manifeste/Parameter unverändert lassen. Kein hörbarer Inhalts- oder Ankernachweis aus Metadaten ableiten.

Das Ergebnis der anschließenden begrenzten Bestandsprüfung steht unten.

## Ergebnis und veröffentlichter separater Zusatz

Die Metadatenprüfung ergab vier passende DE-Testclips und einen passenden EN-Testclip. Test hat deshalb in beiden Sprachen Vorrang; keine Dev-Auswahl. Die zuerst formulierte kanonische Regel wird beibehalten, auch wenn ein späterer DE-Kandidat bereits ohne Padding genau 30 s lang ist.

| Split | Kanonische passende Satz-IDs mit Quellframes |
|---|---|
| DE test | 1732: 471360; 1733: 465600; 1852: 480000; 1899: 471360 |
| EN test | 1670: 468800 |
| DE dev, nicht verwendet | 1550: 473280; 1591: 476160; 1608: 467520 |
| EN dev, nicht verwendet | 1604: 465600 |

### Die zwei fest ausgewählten Fälle

| Merkmal | Deutsch | Englisch |
|---|---|---|
| Neue Case-ID | `fleurs-de-test-1732-thirty-second-v1` | `fleurs-en-test-1670-thirty-second-v1` |
| Ursprünglicher Tar-Pfad | `test/13834375742030049582.wav` | `test/2508559905589212221.wav` |
| Quellframes / Dauer | 471360 / 29,46 s | 468800 / 29,30 s |
| Angefügte Nullframes / Dauer | 8640 / 0,54 s | 11200 / 0,70 s |
| Ergebnis | 480000 Frames, exakt 30 s, PCM16 mono 16 kHz | 480000 Frames, exakt 30 s, PCM16 mono 16 kHz |
| Unabhängige Referenz | unveränderter FLEURS-Rohtext aus TSV-Spalte 3 (nullbasierter Index 2), 31 Whitespace-Wörter | unveränderter FLEURS-Rohtext aus TSV-Spalte 3 (nullbasierter Index 2), 52 Whitespace-Wörter |
| Vorherige Satz-/Clip-Exposition | kein vorhandener Einzelfall und kein Bestandteil des konstruierten 300-s-Falls | derselbe Audioclip bereits `fleurs-en-test-1670`; Übersetzung derselben Satz-ID außerdem `fleurs-de-test-1670`, beide Acceptance; dieselbe Satz-ID zudem als deutsche Aufnahme im konstruierten 300-s-Fall (kein EN-Audioclip-Wiedergebrauch dort) |

Die vollständigen öffentlichen Referenztexte stehen ausschließlich im neuen Manifest. Ihre Herkunft ist der ursprüngliche Publisher-TSV, nicht ein Modelloutput. `clean` übernimmt die bisherige Klassifikation natürlicher, unveränderter FLEURS-Lesesprache; es ist kein neuer Hörnachweis. Der englische Fall erweitert die exakte Aufnahmedauer, aber nicht die unabhängige Satzabdeckung. Die DE-Referenz war nicht im bisherigen Fallmanifest; daraus wird keine nachgewiesene historische Trainingsunabhängigkeit des Modells abgeleitet.

### Quellenpins und tatsächliche WAV-Hashes

Gemeinsame Revision: `google/fleurs` **`70bb2e84b976b7e960aa89f1c648e09c59f894dd`**. Die offiziellen Pfade und getrennten Rechte für Audio/Referenz stehen im bestehenden [Quellenbericht](corpus-sources.md) und [Korpus-README](../../tests/fixtures/corpus/README.md): FLEURS-Audio CC BY 4.0, FLORES-Referenztexte CC BY-SA 4.0, Attribution und Änderungen bewahren.

| Datei/Objekt | Bytes | SHA-256 |
|---|---:|---|
| `artifacts/corpus/source/de_de-test.tar.gz` | 568734559 | `e86b42dfcdef749926cd92135045f87c25966c09e50d01c401adb04ee7d8628f` |
| `artifacts/corpus/source/de_de-test.tsv` | 559542 | `82fab72c58a347345675c238f2492bb997105a4ab45d69e2a2b34ed29082fa97` |
| DE Original-WAV, vollständiges Tar-Member | 1885498 | `51eb452a100223c490096af6458be03ab7997ffe3ae25ad6109fcd8dd306136d` |
| `artifacts/corpus/source/en_us-test.tar.gz` | 289851356 | `d9c2e37b41aacd41bc283554a0a82b5476b36887049774ecb2819dcaaa55a356` |
| `artifacts/corpus/source/en_us-test.tsv` | 367864 | `74c046239374deeb60fa63f258f907388093a32bcaa3140965f70ef05c79f7ca` |
| EN Original-WAV, vollständiges Tar-Member | 1875258 | `6dc676eae80109d088233f838226d4164f76befd8aafc772e1a6cb9eaa0d31df` |
| `artifacts/corpus/audio/fleurs-de-test-1732-thirty-second-v1.wav` | 960044 | `045f4e0c1534e2d4240a10a2dbda1fad22c8ae9ca83809b2cd34e0a2a7510a2e` |
| `artifacts/corpus/audio/fleurs-en-test-1670-thirty-second-v1.wav` | 960044 | `ce7e66bfe03c6d4ce0a1975f7f399d873de5c412635c68fceb86331726bdcb96` |
| `tests/fixtures/corpus/thirty-second-v1.json` | 7209 | `93874aa60fcc63e625f3310a7e5602a7aaa057c7586386e803ac4a5205d12dc4` |

Original-WAV-Bytezahlen ergeben sich aus den tatsächlich enthaltenen RIFF-Quelldaten; keine komprimierten Ausschnitte wurden aus dem Archiv übernommen. Der Quellframecount wurde gegen den wirklichen Float32-`data`-Chunk geprüft.

### Verifikation und Reproduktion ohne Modell

Für jede Sprache wurden die festen `corpus.PINS` geprüft und folgende bestehenden Helfer aus `scripts/corpus.py` direkt verwendet: `parse_tsv` → Dauerfilter/kanonisches erstes Ergebnis → `open_verified` → `read_selected` → `decode_f32_wav` → `pad_pcm(..., 480000)` → `pcm_wav` → `_publish`. `open_verified` hält den geprüften Archiv-FD offen; `read_selected` prüft außerdem den tatsächlich gelesenen komprimierten Bytestrom. Es wurde nichts per `extractall` entpackt.

Vor der Veröffentlichung aller Ausgaben wurden für beide Fälle geprüft: tatsächliches WAV-Format/Framecount mit dem unabhängigen Standardbibliotheks-WAV-Leser, identischer kompletter PCM-Quellprefix, ausschließlich Nullbytes im Padding. Die gebündelten maximal zwei WAVs und das Manifest wurden mit dem existierenden atomaren No-replace-Publisher als 0600-Dateien angelegt. Identische Dateien können erneut geprüft werden; abweichende eingefrorene Ausgaben werden nicht ersetzt.

Das neue Manifest referenziert die unveränderte Basis `tests/fixtures/corpus/manifest.json` mit SHA-256 `734199a55fe70bd0cf0dbc7df2fb3ea7afc7de14ed4daf20aa3d0a5672f0c60b`. Kalibrierungsmanifest, ASR-Parameter und alte WAVs wurden nicht geändert. Es gab **keine ASR, keine Builds und keine Downloads**. Der Zusatz belegt die exakte vollständige Audiodauer; Originalhören, Referenzanker, reale Mikrofon-/Hotkey-Abnahme und ein Modellresultat sind weiterhin gesondert offen.
