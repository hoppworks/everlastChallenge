# P05/A22: Quelle für tatsächlich gesprochenes „Vielen Dank fürs Zuschauen“

Stand: 17.10.2026. Begrenzte Hintergrundrecherche nach dem Research-Skill. Gelesen wurden vorhandene öffentliche Referenztexte, Primärseiten und kleine Metadaten/Untertitel. Keine Audio- oder Modelldownloads, keine ASR, keine Anmeldung, keine Rechteanfrage und keine Änderung eingefrorener Korpusdateien. Videoabfragen unten waren ausschließlich HTTP **HEAD**, ohne Medienkörper. Die untersuchte Aufnahme wurde nicht gehört.

**Ergebnis:** Kein bereits lokal vorhandener oder vollständig qualifizierter kleiner öffentlicher Audioclip gefunden. Es gibt einen konkreten offiziellen Text-/Sprecher-/Zeitcode-Treffer bei der bpb; dessen Lizenz genügt nicht als vorbehaltlose Freigabe eines zugeschnittenen, allgemein weiterverteilbaren Testfixtures. Ein verifizierter Audio-/Inferenznachweis für A22 liegt aus dieser Recherche noch nicht vor. Die konkrete lokale Nutzbarkeit des bpb-Treffers ist ungeklärt. Dieses Ergebnis ist kein Beleg, dass eine geeignete Quelle weltweit nicht existiert.

## Lokaler, reproduzierbarer Nichttreffer

Die vier vorhandenen FLEURS-TSVs wurden vollständig nach Unicode-NFC und Kleinschreibung durchsucht. Geprüft wurden die Zielphrase, Schreibungen mit geradem/typografischem Apostroph, die ausgeschriebene Variante mit „für das“ sowie das einzelne Wort `zuschauen`: jeweils **null Treffer**. Es wurden keine Inferenzergebnisse als Referenz herangezogen.

| Lokale Originalreferenz | Zeilen | SHA-256 |
|---|---:|---|
| `artifacts/corpus/source/de_de-dev.tsv` | 363 | `055b6ddecc9ad493d9ead86babe3ebe225148dd3c174fa6f95cb83a4f9586e18` |
| `artifacts/corpus/source/de_de-test.tsv` | 862 | `82fab72c58a347345675c238f2492bb997105a4ab45d69e2a2b34ed29082fa97` |
| `artifacts/corpus/source/en_us-dev.tsv` | 394 | `9d57ee7e91e9d4c92edb39f6bbea668ef8dc2a3ff96eb510d5580b2ad05d17ec` |
| `artifacts/corpus/source/en_us-test.tsv` | 647 | `74c046239374deeb60fa63f258f907388093a32bcaa3140965f70ef05c79f7ca` |

Das sind 2.266 TSV-Zeilen; der Quelldatensatz ist im bestehenden [Quellenbeleg](corpus-sources.md) auf FLEURS-Revision `70bb2e84b976b7e960aa89f1c648e09c59f894dd` festgelegt. Der Nichttreffer betrifft diese lokal verfügbaren Splits, nicht sämtliche FLEURS-Sprachen oder unbeschaffte Trainingsdaten.

Auch die ausschließlich gelesenen `reference`-Felder der bereits eingefrorenen Manifeste enthalten kein `zuschauen`:

- `tests/fixtures/corpus/manifest.json`: 124 Fälle, SHA-256 `734199a55fe70bd0cf0dbc7df2fb3ea7afc7de14ed4daf20aa3d0a5672f0c60b`.
- `tests/fixtures/corpus/calibration-v2.json`: 42 Fälle, SHA-256 `514befba28e7ea14882cd13a71f971e5fb6c6131f914491b697741a027611a49`.

Die ebenfalls vorhandenen HD-Ziffernlabels und Ja/Nein-Metadaten bieten keine Referenz für den ganzen Satz. Keine Annahme einer durch Verkettung entstandenen natürlichen Aufnahme.

## Konkreter Lead: bpb, Medien-ID 214946

[Offizielle Primärseite: Info Islam – Was bedeutet Dschahiliyya?](https://www.bpb.de/mediathek/reihen/begriffswelten-islam/214946/info-islam-was-bedeutet-dschahiliyya/) benennt **Mirko Drotschmann** als Sprecher, Produktion **28.10.2015**, Dauer etwa sechs Minuten. Die veröffentlichte Volltextalternative enthält die Zielphrase. Dies ist eine redaktionell bereitgestellte Referenz unabhängig von unserem Decoder; eine dokumentierte manuelle Wortprüfung wird daraus nicht behauptet.

Der auf der Seite verlinkte [offizielle deutsche WebVTT](https://www.bpb.de/system/files/asset_mediathek_untertitel_vtt_files/untertitel_filmcms_1259_0.vtt) ist **9.793 Bytes** groß, SHA-256 **`cedc03e6f4d595ededf471a18e7d4bc525e423bbeba95ecc57f39791e24d7159`**. Cue **00:05:33.760 → 00:05:37.160** enthält den Satzanfang und anschließenden Kommentaraufruf. Das Cue ist keine genaue phonemische Satzgrenze; ein isolierter Ausschnitt ist noch nicht bestimmt.

Audio **und** Text werden auf der Seite unter **CC BY-NC-ND 3.0 DE** angeboten. Die [primäre Lizenzbeschreibung](https://creativecommons.org/licenses/by-nc-nd/3.0/de/) beschränkt kommerzielle Nutzung und die Verbreitung bearbeiteter Fassungen. Daraus wird keine Freigabe für ein geschnittenes, allgemein weiterverteilbares Projektfixture abgeleitet; bloße Formatwandlung und gesetzliche Ausnahmen sind gesondert zu beurteilen.

### Beschaffungsmetadaten, noch kein eingefrorenes Audio

Aus der offiziellen HTML-Playerkonfiguration ausgelesen, am Recherchedatum nur per HEAD geprüft:

| Offizieller Medienlink | Content-Length | Last-Modified | ETag |
|---|---:|---|---|
| [H.264 720p](https://mediathek.bpb.de/video/1/214946/mp4-h264-low.mp4) | 98.316.539 Bytes | 05.02.2024 12:55:59 GMT | `"5dc30fb-60c6a992efcff"` |
| [AV1 720p](https://mediathek.bpb.de/video/1/214946/mp4-av1-low.mp4) | 71.537.506 Bytes | 02.11.2024 01:51:57 GMT | `"4439362-60c6a992efcff"` |

Beide HEAD-Antworten waren HTTP 200, `video/mp4`. Es wurde kein kleiner separater Audioclip belegt. Medien-ID und diese Validatoren sind keine unveränderliche Inhaltsrevision und kein Audio-SHA. Der nur im RAM gelesene HTML-Abruf hatte 171.111 Bytes, SHA-256 `4d60c9e25f0da06ca362ed0d1a476e1c2c0bf7f98f6f4de00767efa0e4d1fa7b`; das ist ein Abrufbeleg, keine Zusicherung einer dauerhaft identischen dynamischen Seite.

## Weitere Primärtreffer und präzise Grenzen

| Quelle | Befund | Warum noch kein A22-Fixture |
|---|---|---|
| [TIB AV-Portal 70557 / DOI 10.5446/70557, Spezielle Relativitätstheorie](https://av.tib.eu/media/70557) | Veröffentlichtes Transkript enthält die Zielphrase im Abschnitt ab 09:40; Deutsch, Martin Erdmann, Veröffentlichung 2025. | Portal nennt **CC BY-NC-ND 3.0 DE**. Manuelle Referenzprüfung, konkret menschlicher Sprecher, unveränderliche Medienbytes und kleiner Einzelabruf nicht belegt. Nicht aus dem Text allein als natürliche menschliche Aufnahme qualifizieren. |
| [Commons: deutsche Untertitel zum Kobe-Nachtspaziergang, Revision 1144153632](https://commons.wikimedia.org/w/index.php?title=TimedText:4K_Kobe_Japan_Night_Walk_Tour_at_Merikenpark_-_Stroll_in_the_Park_with_Beautiful_Lights_After_Sunset.webm.de.srt&oldid=1144153632) | Exakte Phrase bei 30:06,480–30:09,480. | Ein deutscher Untertitel beweist kein gesprochenes Deutsch. [Dateiseite, Revision 1031368401](https://commons.wikimedia.org/w/index.php?title=File:4K_Kobe_Japan_Night_Walk_Tour_at_Merikenpark_-_Stroll_in_the_Park_with_Beautiful_Lights_After_Sunset.webm&oldid=1031368401) beschreibt einen japanischen Rundgang, Original **2.530.401.636 Bytes**; Lizenzprüfung des externen Imports ist als offen markiert. Kein Download. |
| [Offizielles Mozilla-HF-Repository Common Voice 17](https://huggingface.co/datasets/mozilla-foundation/common_voice_17_0) | Aktuell leerer Datensatz; Publisher verweist seit Oktober 2025 ausschließlich auf Mozilla Data Collective. | Kein durchsuchbarer offizieller deutscher Parquet-Bestand und kein passender Clip/Referenzdatensatz daraus nachgewiesen. Dies ist ein Zugangs-/Metadatenbefund, kein geprüfter Nichttreffer im vollständigen Common-Voice-Korpus. |
| [MDC Common Voice Spontaneous Speech 2.0 – German, ID cmj8u489v001tnxzp7x8ayacr](https://datacollective.mozillafoundation.org/datasets/cmj8u489v001tnxzp7x8ayacr) | Indexierte Primärmetadaten nennen CC0, MP3/TSV und etwa 21,96 MB. Live-HTML liefert nur einen Ladeplatzhalter; kein einzelner Zielphrasen-Treffer. | Nicht als verifizierter aktueller Pin oder Downloadkandidat übernommen. [Offizielle API-Anleitung](https://mozilladatacollective.com/api-reference) verlangt API-Zugang und Zustimmung zu Datasetbedingungen; hier keine Anmeldung oder Zustimmung vorgenommen. |

Die [offizielle MDC-FAQ](https://community.mozilladatacollective.com/faq-can-i-get-the-common-voice-or-other-mdc-datasets-from-other-platforms-like-github-or-hugging-face/) bestätigt die exklusive Bereitstellung und begründet sie mit Einwilligungswiderrufen/aktualisierten Datenständen. Alte fremde Mirrors wurden deshalb nicht als aktuelle offizielle Quelle oder Rechtebeleg übernommen.

Commons-Such-API, Namespace 0/6, exakte Phrase: [konkrete Abfrage](https://commons.wikimedia.org/w/api.php?action=query&list=search&srsearch=%22Vielen%20Dank%20f%C3%BCrs%20Zuschauen%22&srnamespace=0%7C6&srlimit=10&format=json) lieferte `totalhits: 0`. Der separat über Websuche gefundene TimedText-Namespace ist davon ausdrücklich nicht umfasst. Der versuchte frühere Lingua-Libre-APIpfad `https://lingualibre.org/api.php` lieferte lediglich die neue HTML-Apphülle, keinen auswertbaren Suchbefund. Gezielte Websuchen für Lingua Libre, OpenHPI, Freesound und die exakte Phrase ergaben keinen verifizierbaren passenden Primärclip; das ist keine vollständige Korpusabfrage. Übersetzte Webseiten, bloße Abspanntexte und automatisch transkribierte Fremdportale wurden nicht als unabhängige deutsche Audio-Referenz übernommen.

## Empfohlenes Vorgehen ohne Acceptance-Leakage

Die folgende Quellenqualifizierung ist eine Rechercheempfehlung, keine zusätzliche Planpflicht oder Sperre für ASR. Der Plan verlangt einen tatsächlich gesprochenen legitimen Satz und passend lizenzierte lokale Fixtures; er schreibt weder eine menschliche Quellenkategorie noch ein Hör-before-ASR-Verfahren oder allgemeine Weiterverteilbarkeit jedes lokalen Fixtures vor. Auch bei NC/ND ist die konkrete lokale Nutzung gesondert zu klären; fehlende pauschale Weitergabefreigabe allein verbietet nicht jede lokale Prüfung.

1. Vor weiterer Beschaffung einen getrennten A22-Zusatz mit der vorgegebenen Referenz festlegen. Bestehende 90 Acceptance-Fälle und sechs zentrale Zeitmessfälle unverändert lassen; die zusätzliche gezielte Prüfung nicht rückwirkend als Teil ihres ursprünglichen WER-Gates ausgeben.
2. Bevorzugt eine einzelne ausdrücklich lizenzierte menschliche Originalaufnahme des Satzes oder einen belegten offiziellen CC0/CC-BY-Clip beschaffen. Bei der bpb wäre zuvor die konkrete geplante Nutzung einschließlich Ausschnitt/Weitergabe zu klären; diese Recherche hat niemanden kontaktiert.
3. Erst danach Originalbytes und Referenzmetadaten pinnen, ohne ASR eine vollständige hörbare Satzgrenze bestimmen und menschliches Originalhören ausdrücklich protokollieren. Vorab gewählte Zusatzreferenz und mögliche Kontextwörter erhalten; keine Fragmente zu einem vermeintlich natürlichen Satz verkleben und keine TTS-Aufnahme so benennen.
4. Produktive Inferenz getrennt prüfen: Der echte Satz darf nicht durch generische Halluzinationsfilter entfernt werden. Ein deterministischer Filter-/Controller-Test belegt die Filterregel, aber keine Audio-/ASR-Eigenschaften des noch ungeprüften Clips. Die tatsächliche P05/A22-Abnahme bleibt offen.
