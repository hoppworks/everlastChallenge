# P05: Quellen für einen eingefrorenen DE/EN-Prüfkorpus

Recherche: 17.10.2026. Untersucht wurden Primärquellen und kleine Metadatendateien. Für diese Recherche wurden keine Audioarchive, Modelle oder zusätzlichen Abhängigkeiten aus dem Netz geladen und keine Transkriptionen erzeugt. Der HD-Nachtrag unten prüft ein inzwischen vom Hauptauftrag lokal bereitgestelltes Archiv. Die folgenden Quellen sind Beschaffungskandidaten, noch kein vollständig abgenommener Korpus.

## Entscheidung und verbleibende Abdeckung

**FLEURS ist als Basis für gelesene deutsche und englische Sätze geeignet.** Seine Referenztexte existieren unabhängig von unserem ASR-Modell. Es ersetzt weder natürliche Kurzäußerungen von 0,2–1 s noch einen geprüften fünfminütigen Fall ohne sichere Pause. Die kleinste unkomplizierte Beschaffung sind einzelne Sprach-/Splitarchive, nicht das gesamte Dataset. Für kurze deutsche und englische Ziffern ist Heidelberg Digits ein Kandidat; für einen sehr kleinen englischen Download bietet FSDD einzelne gepinnte WAV-Dateien. Englische Namen und „no“ bietet Speech Commands. Eine vollständig belegte kleine Quelle für **deutsche kurze Namen und Negationen** wurde hier nicht gefunden.

Die Korpusanforderungen bleiben IMPLEMENTATION_PLAN.md, §13 (historischer oder lokaler Beleg, nicht mitgeliefert): mindestens 30 DE-/20 EN-Fälle und zusammen 1500 Referenzwörter, getrennte Kalibrierungs-/Abnahmemengen, Anker, Stille und schwierige Fälle. Die geplante Auswahl von 40 DE-/30 EN-Abnahmefällen plus je zehn Kalibrierungsfällen ist vor Inferenz festzuschreiben; Fallzahlen allein beweisen weder Wortzahl noch Klassenabdeckung.

## Google FLEURS: Version, Dateien und Referenzen

Offizieller Publisher: Google, Dataset `google/fleurs`. Geprüfter Commit: **`70bb2e84b976b7e960aa89f1c648e09c59f894dd`**, aufgelöst über die [offizielle Revisions-API](https://huggingface.co/api/datasets/google/fleurs/revision/70bb2e84b976b7e960aa89f1c648e09c59f894dd). Für Abrufe immer diesen Commit verwenden, nicht `main` oder eine bewegliche Konvertierungsbranch.

Die Autoren beschreiben natürliche Aufnahmen vorgelesener FLORES-Sätze, zusätzliche menschliche Prüfung der Übereinstimmung und vorbestehende, menschlich übersetzte Ausgangstexte. Originaltext, normalisierte Fassung und Zeichenfassung werden getrennt geliefert; das ist keine ASR-generierte Referenz. Aufnahmeprüfung verhindert allerdings nicht jeden möglichen Text-/Audiofehler in einer lokalen Teilmenge. [FLEURS-Paper, §§2.1–2.2](https://arxiv.org/html/2205.12446v1#S2)

Die headerlosen TSV-Dateien haben sieben Spalten: `id`, `filename`, `raw_transcription`, normalisierte `transcription`, Zeichenfassung, `num_samples`, `gender`. Als unveränderte Referenz ist Spalte 3 zu übernehmen; die FLEURS-Normalisierung entspricht nicht automatisch der projektspezifischen WER-Normalisierung. Dieselbe Satz-ID kann mehrere Aufnahmen bezeichnen: für die Auswahl IDs deduplizieren und für jeden gewählten Fall zusätzlich Dateiname/Split festhalten. [Gepinnte deutsche Testmetadaten](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/de_de/test.tsv), [englische Testmetadaten](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/en_us/test.tsv)

Eigene Auszählung der unveränderten TSVs; Dauern hier nur `num_samples / 16000`, noch keine WAV-Prüfung:

| Gepinnte Datei | Bytes | Zeilen | Dauer min–max | 0,2–1 s |
|---|---:|---:|---:|---:|
| [DE dev.tsv](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/de_de/dev.tsv) | 221691 | 363 | 3,84–38,82 s | 0 |
| DE test.tsv, Link oben | 559542 | 862 | 3,60–47,94 s | 0 |
| [EN dev.tsv](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/en_us/dev.tsv) | 213065 | 394 | 2,70–31,74 s | 0 |
| EN test.tsv, Link oben | 367864 | 647 | 3,60–29,30 s | 0 |

TSV-SHA256, in derselben Reihenfolge:

```text
055b6ddecc9ad493d9ead86babe3ebe225148dd3c174fa6f95cb83a4f9586e18
82fab72c58a347345675c238f2492bb997105a4ab45d69e2a2b34ed29082fa97
9d57ee7e91e9d4c92edb39f6bbea668ef8dc2a3ff96eb510d5580b2ad05d17ec
74c046239374deeb60fa63f258f907388093a32bcaa3140965f70ef05c79f7ca
```

Die Metadaten enthalten auch längere Fälle als die im Paper genannte 30-s-Grenze. Deshalb tatsächliche WAV-Samplerate, Kanalzahl, Framezahl und Dauer messen; eine TSV-Dauer ist kein bestandener 30-s-Test.

### Begrenzte Beschaffung

Die [offiziellen DE-Audiometadaten](https://huggingface.co/api/datasets/google/fleurs/tree/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/de_de/audio?expand=false) und [EN-Audiometadaten](https://huggingface.co/api/datasets/google/fleurs/tree/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/en_us/audio?expand=false) nennen folgende Archive. SHA256 sind die dort veröffentlichten LFS-Objekt-IDs, noch keine eigene Prüfung heruntergeladener Archive.

| Archiv unter gepinntem Commit | Bytes | SHA256/LFS-OID |
|---|---:|---|
| [DE dev](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/de_de/audio/dev.tar.gz) | 227694463 | `72cac13bbddbddf674d8102a9e7b90a3b1db5d777ed92e88eccc5ae69cb8fc9d` |
| [DE test](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/de_de/audio/test.tar.gz) | 568734559 | `e86b42dfcdef749926cd92135045f87c25966c09e50d01c401adb04ee7d8628f` |
| [EN dev](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/en_us/audio/dev.tar.gz) | 171250900 | `2658fda72f199e12676ecac9415094667a4e14e149b146e568ea00b2a2f0954c` |
| [EN test](https://huggingface.co/datasets/google/fleurs/resolve/70bb2e84b976b7e960aa89f1c648e09c59f894dd/data/en_us/audio/test.tar.gz) | 289851356 | `d9c2e37b41aacd41bc283554a0a82b5476b36887049774ecb2819dcaaa55a356` |

Beide dev-Archive zusammen: 398945363 B; beide test-Archive: 858585915 B; alle vier: 1257531278 B. Train-Archive werden für diese Auswahl nicht benötigt. Einzelne WAV-Dateien liegen in der untersuchten Hauptrevision nicht als separat abrufbare Repositorydateien vor.

Die offizielle [Viewer-Dokumentation](https://huggingface.co/docs/dataset-viewer/rows) beschreibt Einzelzeilenabrufe, dokumentiert dafür aber keinen Revisionsparameter. Am Recherchetag antwortete `/rows` für EN/test mit HTTP 500, `/first-rows` für DE/test mit 501. [Parquet-Metadaten](https://datasets-server.huggingface.co/parquet?dataset=google/fleurs) waren verfügbar: DE/test 726207372 B, EN/test 401722686 B; dev 290966490 bzw. 236549523 B. Vollständige Parquetdownloads sind somit keine kleinere Alternative. Range-/Rowgroup-Zugriff oder Viewer-Audioassets können spätere Optimierungen sein, sind hier aber nicht als funktionierender, an die Originalrevision gebundener Abruf belegt. `refs/convert/parquet` ist eine eigene bewegliche Branch; zufällig passende Zeilenindizes oder ein aktueller Viewer-Link beweisen die Herkunft nicht.

### Lizenz und Attribution getrennt behandeln

Die [gepinnten Google-Metadaten](https://huggingface.co/datasets/google/fleurs/blob/70bb2e84b976b7e960aa89f1c648e09c59f894dd/README.md) deklarieren `cc-by-4.0`; ihr Fließtext verweist allgemeiner auf CC-BY. Die zugrunde liegenden FLORES-101-Texte haben eine eigene Herkunft: Das [offizielle FLORES-101-README vom 16.05.2022](https://github.com/facebookresearch/flores/blob/0c4f1deac29faff43573482f29cd0060b3fccaf7/README.md) nennt CC-BY-SA; die dazugehörige [Lizenzdatei](https://github.com/facebookresearch/flores/blob/0c4f1deac29faff43573482f29cd0060b3fccaf7/LICENSE) spezifiziert Attribution-ShareAlike 4.0. Deshalb keine pauschale Aussage „alle Audios und Referenztexte ausschließlich CC BY 4.0“ aus dem HF-Tag ableiten.

Für die lokalen Herkunftsdaten getrennt erfassen: FLEURS/Google, Conneau et al. 2022, Dataset-Commit, Originaldatei und deklarierte [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); FLORES-101/Goyal et al., Textquelle und [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Bei veröffentlichten Ausschnitten oder zusammengesetzten Fixtures zusätzlich Änderungen und jeweilige Attribution erhalten. Diese Recherche belegt Publisherangaben, keine abschließende Klärung einer abweichenden Lizenzierung abgeleiteter Texte.

## Natürliche Kurzäußerungen und Anker

| Quelle | Belegbare Eignung | Beschaffung und Grenze |
|---|---|---|
| **Heidelberg Digits, Originalaudio v1.0** | Deutsch und Englisch, natürliche isolierte Ziffern mit vorgegebenen Labels statt ASR-Pseudoreferenzen. [Autorenpaper §2.1.1](https://arxiv.org/html/1910.07407v3#S2.SS1.SSS1). Die Roh-Audio-Lizenz ist jetzt durch die archivinterne README als **CC BY 4.0** belegt. | [Offizielles Audioarchiv](https://zenkelab.org/datasets/shd/hd_audio.tar.gz), lokal vollständig vorhanden: **337342212 B**. Prüfsummen, tatsächliche FLAC-Formate, Splits und kurze Kandidaten stehen im HD-Nachtrag unten. Keine selektive Einzeldatei-Schnittstelle belegt; für ASR Originalaudio verwenden, keine SHD-Spike-`.h5`-Dateien. |
| **Google Speech Commands v0.02** | Englische natürliche Einzelwörter bis etwa eine Sekunde; Ziffern, „Marvin“, „Sheila“, „no“. Sprecher lasen Vorgaben, zusätzliche menschliche Prüfung entfernte Abweichungen. Das ist keine ASR-Pseudoreferenz. Autoren geben CC BY 4.0 an. [Google-Autorenpaper §§5–6](https://arxiv.org/html/1804.03209v1) | [Offizielles v0.02-Archiv](https://storage.googleapis.com/download.tensorflow.org/data/speech_commands_v0.02.tar.gz), rund 2,7 GB laut Paper. Kein kleiner offizieller Einzelabruf hier belegt. Die [TFDS-Standardabbildung](https://www.tensorflow.org/datasets/catalog/speech_commands) auf zwölf Klassen fasst zusätzliche Wörter als „unknown“ zusammen; für Namen/Zahlen Originalwortlabels bewahren. |
| **Free Spoken Digit Dataset v1.0.10** | Natürliche englische Ziffern, CC BY-SA 4.0, Dateiname als Label. Originales [Erfassungsskript](https://github.com/Jakobovski/free-spoken-digit-dataset/blob/d6938f9bf1545aa66d8489fc9f1385a7abd64282/acquire_data/say_numbers_prompt.py) und [Labelskript](https://github.com/Jakobovski/free-spoken-digit-dataset/blob/d6938f9bf1545aa66d8489fc9f1385a7abd64282/acquire_data/split_and_label_numbers.py) dokumentieren vorgegebene Zahlenfolgen. | [Maintainer-Repository, gepinnter Commit](https://github.com/Jakobovski/free-spoken-digit-dataset/tree/d6938f9bf1545aa66d8489fc9f1385a7abd64282), [Release v1.0.10](https://github.com/Jakobovski/free-spoken-digit-dataset/releases/tag/v1.0.10). Einzelne WAVs über `raw.githubusercontent.com/Jakobovski/free-spoken-digit-dataset/<commit>/recordings/<filename>` beschaffbar; deutlich kleiner als Vollarchive. 8-kHz-Quelle, variable Dauer: 0,2–1 s gezielt auswählen und Produktionsresampling verwenden. Keine deutschen Fälle/Namen/Negationen. |

### HD: Nachtrag aus dem tatsächlich vorhandenen Originalarchiv

Am 17.10.2026 wurde `artifacts/corpus/source/hd_audio.tar.gz` ausschließlich lesend mit `tarfile` geprüft; kein vollständiges Entpacken, keine Inferenz. Die lokale Dateigröße beträgt **337342212 B**. Der berechnete MD5 **`e95704b1a726235e3c679baa81bb49c7`** stimmt mit [der offiziellen Prüfsummenliste](https://zenkelab.org/datasets/shd/md5sums.txt) überein. Zusätzlich berechneter SHA256: **`4980128f02deba417ad3592e37166b8af603e200e298dd6108c20c4ffaa2e610`**. Damit lässt sich die konkrete v1.0-Ausgabe unabhängig vom unveränderten URL-Namen festhalten.

Das Archiv enthält **10420 FLAC-Dateien**, `README.md` (916 B), `train_filenames.txt`, `test_filenames.txt` und den Verzeichniseintrag `audio`. Die archivinterne README nennt ausdrücklich das **spoken digits dataset**, Copyright 2019 Benjamin Cramer, Yannik Stradmann, Johannes Schemmel und Friedemann Zenke, und verlinkt dafür [Creative Commons BY 4.0](https://creativecommons.org/licenses/by/4.0/). Die zuvor offene Zuordnung dieser Lizenz zum ursprünglichen Audio ist damit geklärt; es handelt sich nicht nur um eine Erklärung für Spike-Daten. Die README verlangt die Zitierung von Cramer et al., [DOI 10.1109/TNNLS.2020.3044364](https://doi.org/10.1109/TNNLS.2020.3044364).

Dateimuster: `audio/lang-{english|german}_speaker-{00..11}_trial-{n}_digit-{0..9}.flac`. Sprache, anonymisierte Sprecher-ID, Versuch und Ziffer stehen im Dateinamen; weitere Sprecheridentitäten oder wortgetreue Satztranskripte liegen nicht im Archiv. Die Listen führen Dateinamen **ohne** `audio/`-Präfix: train 8156 Zeilen, test 2264 Zeilen, ohne Überschneidung zwischen den beiden Listen. Nicht allein aus einem Sprecherpräfix auf den Split schließen: beispielsweise liegt `lang-english_speaker-00_trial-0_digit-8.flac` im Testsplit, andere Sprecher-00-Dateien im Training.

Die folgenden Werte wurden aus dem FLAC-`STREAMINFO`-Block aller Dateien gelesen: Samplerate, Kanalzahl, Bittiefe und Gesamtframes, Dauer = Frames/Samplerate. **Alle veröffentlichten FLACs sind 48000 Hz, mono, 16 Bit**; eine andere Bittiefe ursprünglicher Studioaufnahmen aus dem Paper ersetzt diese konkrete Dateiprüfung nicht. Es wurde dabei kein Audio dekodiert oder gehört.

| Sprache | Clips | Dauer min–max | Clips in 0,2–1,0 s | Sprecher-IDs |
|---|---:|---:|---:|---|
| Deutsch | 5330 | 0,396542–1,372333 s | 5087 | 00–11 |
| Englisch | 5090 | 0,234208–1,160792 s | 5021 | 00–11 |

Vorläufige Kandidaten für eine spätere Builder-Erweiterung, **noch keine eingefrorenen oder gehörten Abnahmefälle**. Alle sechs stehen ausdrücklich in `test_filenames.txt`; Auswahl nur über unabhängige Labels und Dauer, ohne ASR-Ergebnis:

| Archivmitglied | Frames bei 48 kHz | Dauer | FLAC-Bytes | Vorab-Ziffernlabel |
|---|---:|---:|---:|---:|
| `audio/lang-german_speaker-04_trial-0_digit-1.flac` | 42736 | 0,890333 s | 37795 | 1 |
| `audio/lang-german_speaker-04_trial-0_digit-2.flac` | 39723 | 0,827563 s | 35776 | 2 |
| `audio/lang-german_speaker-04_trial-0_digit-8.flac` | 34217 | 0,712854 s | 28998 | 8 |
| `audio/lang-english_speaker-04_trial-0_digit-1.flac` | 31519 | 0,656646 s | 21975 | 1 |
| `audio/lang-english_speaker-04_trial-0_digit-2.flac` | 30824 | 0,642167 s | 22763 | 2 |
| `audio/lang-english_speaker-04_trial-0_digit-8.flac` | 28224 | 0,588000 s | 22924 | 8 |

Ein späteres Manifest muss die **vor Inferenz definierte** Lexikalisierung des Zahlworts mitführen, beispielsweise `eins/zwei/acht` bzw. `one/two/eight`, und die Audio-/Labelübereinstimmung unabhängig prüfen. Die Datei-Ziffer ist keine ASR-Referenzabschrift. Eine nachträgliche Wahl zwischen Ziffer und Zahlwort anhand der Modellantwort wäre keine unabhängige Goldreferenz. Diese Auswahl liefert keine deutschen Namen/Negationen und keinen deutschen 0,2-s-Grenzfall; ihre Mindestdauer und übrige Korpuslücken bleiben sichtbar.

Die deutsche/englische [MLCommons Multilingual Spoken Words Collection](https://mlcommons.org/datasets/multilingual-spoken-words/) bietet eine weitere CC-BY-4.0-Option: natürliches Satz-Audio mit automatisch ausgerichteten Wortausschnitten. Satztexte sind eine unabhängige Quelle, Wortgrenzen aber automatisch ermittelt. Die deutschen/englischen Archive sind mit ca. 17,95/32,45 GB für eine kleine Beschaffung ungünstig; die kleine Microset-Ausgabe enthält nicht Deutsch. Keine weitere Beschaffung daraus vorgenommen.

Attribution der Ergänzungsquellen im späteren Fixturemanifest: Heidelberg Digits — Cramer, Stradmann, Schemmel und Zenke (2020), Originalaudio v1.0 und Laborlink; Speech Commands — Pete Warden (2018), v0.02 und Google-Archivlink; FSDD — Maintainer/Contributors des Jakobovski-Repositories, v1.0.10 und Commit. Jeweils die oben belegte Lizenz und jede Ausschnitt-/Resamplingänderung mitführen; den bei Beschaffung vorgefundenen Lizenzstand sichern.

„no“ allein prüft englische Worterkennung, nicht die Erhaltung einer Negation im Satz. Ziffernklassen müssen vor Inferenz in die tatsächlich gesprochenen Wortreferenzen abgebildet werden. Namen, Zahlen und Negationen in FLEURS-Sätzen können unabhängig über TSV-Texte ausgewählt werden; die Hörprüfung und Ankerannotation bleiben erforderlich. Schriftliche Ziffern gegenüber gesprochenen Zahlwörtern sind besonders zu beachten: §13 erlaubt keine nachträgliche Zahlumwandlung zum Verbessern der WER. Referenzen nicht anhand von Modellhypothesen korrigieren.

## Zusätzlicher deutscher 300-s-Fall

Eine kleine, bereits belegte natürlich durchgesprochene deutsche 300-s-Datei ohne sichere Pause wurde nicht gefunden. FLEURS liefert einzelne Lesesätze, keine zusammenhängenden fünfminütigen Monologe. Eine offen dokumentierte Alternative ist ein **konstruierter Belastungsfall aus natürlichen Aufnahmen**: vor Inferenz fest ausgewählte vollständige DE-Sätze an verifizierten Nichtsprachgrenzen verbinden; jede Quelle/Referenz und alle Framebereiche sowie entfernte Pausen und eingefügte Samples protokollieren. Referenztext ist die festgelegte Verkettung der unabhängigen Quelltexte. Keine Wörter nachträglich aus ASR-Ausgaben übernehmen.

Für genau 4800000 Ausgangsframes bei 16 kHz muss die letzte vollständige Äußerung samt letztem Laut erhalten bleiben; Restauffüllung, Übergänge und gegebenenfalls Pausenkürzungen werden explizit als Transformation markiert. Die gesamte Datei muss gegen die festgelegte sichere Pausendauer geprüft werden, einschließlich interner Originalpausen. Bloßes Aneinanderhängen oder Wiederholen beweist diese Bedingung nicht. Diese Konstruktion prüft Zeitgrenze/Vollständigkeit, gilt jedoch nicht als natürliche kontinuierliche Aufnahme oder Mikrofonabnahme. Leise, verrauschte, wiederholte und stille Fälle sowie reale Geräte-/Hotkey-Proben bleiben gesondert zu qualifizieren.
