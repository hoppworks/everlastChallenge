# Whisper-No-Speech: Quellenprüfung der Logit-Zeile

Stand: 2026-10-17. Read-only Sourceaudit; keine Registryänderung, kein Decoderlauf,
kein Modell-/Audiodownload und keine Änderung an Produktionscode. Der Auftrag
meldet 42 Kalibrierungsfälle mit fast uniformen Sprachwerten um 1,93e-5 und
wesentlich kleineren Rauschwerten. Diese Messung wurde hier nicht wiederholt.

## Ergebnis

**Die vermutete falsche Logit-Zeile ist im gepinnten C++-Code nachgewiesen.**
Beim initialen mehrteiligen Prompt wird nur die letzte Logit-Zeile aus dem
Backend kopiert; die No-Speech-Berechnung liest aber die erste Zeile. Diese ist
bei frisch angelegtem Zustand zunächst null und kann später alte Werte enthalten.
Ein bloßes Umstellen auf die letzte Zeile wäre dennoch keine korrekte Reparatur:
OpenAI berechnet No-Speech aus der **SOT-Zeile**, vor Sprach-/Task-Tokens.

Das belegt einen Quellenfehler, aber noch nicht die numerische Herkunft jedes
einzelnen gemeldeten Kalibrierungswertes. Die jetzigen Werte sind keine belastbare
Grundlage für einen No-Speech-Schwellenwert oder eine inverse Heuristik.

## Exakter Stand und Beweiskette

Lokal geprüft: `whisper-rs 0.16.0`, `whisper-rs-sys 0.15.0` aus
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.
`sys/.cargo_vcs_info.json` nennt whisper-rs-Commit
`7558e1b72f54f2f22a53589afb77e65681834c36`. Dessen Submodul verweist auf
whisper.cpp-Commit **`2eeeba56e9edd762b4b38467bab96c2517163158`**.
[Submodul-Metadaten](https://api.github.com/repos/tazz4843/whisper-rs/contents/sys/whisper.cpp?ref=7558e1b72f54f2f22a53589afb77e65681834c36).

**`d1e041704c12e20b8eb9596632db3b3b31046348dbb0e493f8a5ab8edd5f4376` ist
der SHA256 der Datei `src/whisper.cpp`, kein Git-Commit.** Lokale Datei und
offizielle Raw-Datei dieses Submodul-Pins wurden gehasht und stimmen überein.
[Gepinnte offizielle C++-Quelle](https://raw.githubusercontent.com/ggml-org/whisper.cpp/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp).

Die folgenden Zeilenangaben beziehen sich auf diese unveränderte Datei:

1. **Zeilen 511–522:** `whisper_batch_prep_legacy` setzt zunächst alle
   `batch.logits[i]` auf null und ausschließlich das letzte Token auf eins.
2. **Zeilen 2949–2955:** `whisper_decode_internal` hält
   `n_tokens * n_vocab` Floatplätze vor. Eine Zeile wird nur kopiert, wenn ihr
   Logit-Flag gesetzt ist; die Ausgabe bleibt nach Tokenindex adressiert.
   Nicht angeforderte Plätze werden nicht mit aktuellen Backendlogits gefüllt.
3. **Zeilen 6954–6978, 7111–7150:** Der Prompt enthält SOT und beim
   multilingualen Modell weitere Sprach-/Task-Tokens, gegebenenfalls
   No-Timestamps sowie vorangestellten Kontext. Die Vorbereitung fordert somit
   regelmäßig eine andere Zeile als die SOT-Zeile an.
4. **Zeilen 7151–7160:** Beide Softmax-Helfer erhalten den vollständigen
   `state->logits`-Vektor und `n_vocab`; ihre Schleifen lesen die ersten
   `n_vocab` Werte. Der ausgewählte No-Speech-Tokenwert stammt damit aus Zeile
   null, unabhängig davon, ob diese überhaupt angefordert wurde.
5. **Zeilen 7170, 6140–6171:** Die eigentliche Textdekodierung setzt dagegen
   `i_batch = prompt.size() - 1` und kopiert explizit diese Zeile.
6. **Zeilen 7642/7687 und 8057–8062:** Der berechnete Fensterwert wird in
   Segmentergebnisse übernommen und vom C-Getter unverändert ausgegeben.
   Rusts `WhisperSegment::no_speech_probability()` delegiert an diesen Getter;
   der Getter korrigiert oder invertiert nichts.

[C++-Quelle mit Zeilenankern](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L2949-L2955),
[initiale No-Speech-Berechnung](https://github.com/ggml-org/whisper.cpp/blob/2eeeba56e9edd762b4b38467bab96c2517163158/src/whisper.cpp#L7146-L7172),
[Rust-Segmentgetter](https://docs.rs/whisper-rs/0.16.0/src/whisper_rs/whisper_state/segment.rs.html).

Zusätzlich sucht `whisper_compute_logprobs` in Zeile 6107 das Maximum über den
**gesamten** übergebenen Vektor, summiert danach jedoch nur die ersten
`n_vocab` Werte. Ein Maximum aus einer fremden Zeile ist bei exakter Arithmetik
nur eine gemeinsame Verschiebung; bei Float-Unterlauf kann es die Berechnung
zusätzlich stören. Auch deshalb muss eine Reparatur eine exakt abgegrenzte
Zeile an die bestehenden Helfer geben.

Bei frischem `std::vector<float>` erzeugt die Größenvergrößerung nullgefüllte
Elemente. Eine nullgefüllte erste Zeile führt grundsätzlich zu `1 / n_vocab`:
bei etwa 51.865 Vokabeleinträgen rund 1,928e-5. Diese Größenordnung passt zur
gemeldeten Beobachtung. Spätere Ein-Token-Decodes schreiben dagegen Zeile null;
bei Fallbacks oder weiteren Audiofenstern kann diese anfangs gelesene Zeile
deshalb aus einem früheren Dekodierschritt stammen. Die genaue Zuordnung der
kleinen Rauschwerte erfordert Instrumentierung; eine echte inverse Bedeutung
des No-Speech-Tokens folgt daraus nicht.

## Referenzsemantik von OpenAI

OpenAI-Referenzpin für diesen Audit:
**`86098128c0b4f24f0e2aa2994de830614b474227`**. `DecodingTask` ermittelt
`sot_index` innerhalb der Initialtokens. In der ersten Decoderiteration wird
die unmodifizierte SOT-Zeile über das Vokabular normalisiert und daraus der
No-Speech-Token entnommen. Erst anschließend wird für die Textfortsetzung
die letzte Tokenzeile gewählt. Das gilt auch bei vorangestelltem Promptkontext;
SOT ist dann nicht zwingend Tokenindex null.
[OpenAI decoding.py: SOT-Index](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/whisper/decoding.py#L535-L537),
[OpenAI decoding.py: erste Decoderiteration](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/whisper/decoding.py#L685-L697).

## Kleinstmögliche belastbare Korrektur

Empfehlung für eine spätere autorisierte Implementierung: bestehende Version
beibehalten und einen kleinen, überprüfbaren **lokalen Cargo-Patch von
whisper-rs-sys** mit Lizenz/Pin/Patchnachweis einchecken. Keine Änderung der
globalen Cargo-Registry und kein stiller Dependencywechsel.

- Im ersten Promptdecode den tatsächlichen SOT-Index bestimmen: in diesem
  Kontrollfluss beginnt `prompt_init` mit SOT und wird hinten angehängt;
  entsprechend ist `prompt.size() - prompt_init.size()` der relevante Index.
  Den Index/Token als Invariante prüfen, statt das erste SOT-Vorkommen in
  möglichem historischen Text blind zu wählen.
- Nach `whisper_batch_prep_legacy` zusätzlich das Logit-Flag **dieser** Zeile
  einschalten; das letzte Flag für normale Dekodierung erhalten.
- Für No-Speech genau `n_vocab` Rohlogits dieser SOT-Zeile in einen begrenzten
  lokalen Vektor kopieren und beide unveränderten Helfer darauf anwenden.
  Dadurch wird auch der fremde-Maximum-Effekt beseitigt. Keine Temperatur-
  oder Tokenfilter anwenden und nicht die Decoderzeile austauschen.

Das erfordert keinen zweiten Encoderlauf und keinen Umbau der Textdekodierung;
es fordert eine zusätzliche Backend-Ausgabezeile an. Zusätzliche Zeilentransfers
und Hostkopie bleiben in Vokabulargröße begrenzt. Tatsächliche Zeit-/RAMkosten
sind zu messen, nicht aus dieser Quellenprüfung abzunehmen.

Vor Freigabe nötig: deterministischer Nachweis mit verschieden belegter
SOT-/letzter Zeile, Kontext vor SOT, frischem und wiederverwendetem Zustand;
anschließend derselbe **Kalibrierungs**-Satz zur Differentialmessung. Erst
danach Schwellen anhand unabhängiger positiver und negativer Fälle beurteilen.
Auch ein korrekt berechneter Modellwert ist keine Garantie gegen Halluzinationen.

## Sichere Rust-Lowlevel-API als Diagnoseoption

`WhisperState` bietet sichere Methoden `pcm_to_mel`, `encode`, `decode` und
`get_logits`; `WhisperContext` bietet `token_sot` und `token_nosp`.
Ein isolierter Diagnosezustand kann begrenztes PCM mit `pcm_to_mel` vorbereiten,
`encode(0, threads)` ausführen, **genau ein Token `[SOT]`** mit
`decode(&[context.token_sot()], 0, threads)` dekodieren und danach
`get_logits()` kopieren. Vokabularlänge, endliche Werte und Tokenindex prüfen;
Softmax numerisch stabil über genau diese eine Zeile bilden.
[Versionierte Rust-API](https://docs.rs/whisper-rs/0.16.0/whisper_rs/struct.WhisperState.html),
[versionierter Wrappercode](https://docs.rs/whisper-rs/0.16.0/src/whisper_rs/whisper_state/mod.rs.html).

**Ein Token ist entscheidend:** Die Wrapperdokumentation behauptet zwar einen
letzten Logitvektor, aber der gepinnte C-Getter gibt tatsächlich den Anfang des
Vektors zurück (4193–4199); bei Mehrtoken-Decodes besteht dieselbe erste-Zeile-
Falle. Mit einem einzigen SOT stimmen erste, letzte und benötigte Zeile überein.
Dies ist eine ausdrücklich **promptfreie** Diagnose. Sprach-/Task-Tokens nach
SOT verändern dessen Zeile wegen der kausalen Attention nicht; historischer
Prompt vor SOT kann sie dagegen beeinflussen und fehlt im isolierten Probe.
Der Diagnosezustand sollte nicht nebenläufig zur laufenden Textdekodierung
mutiert werden.
[OpenAI Initialtokens mit optionalem Vorprompt](https://github.com/openai/whisper/blob/86098128c0b4f24f0e2aa2994de830614b474227/whisper/decoding.py#L587-L613).

Das ist eine **Diagnoseoption, kein kostenloser Produktionsschutz**: zusätzlich
zur vorhandenen `full()`-Transkription entstehen Mel-Berechnung, ein kompletter
Encoderlauf und ein Decoderlauf pro geprüftem Fenster sowie Zustands-/KV-Speicher.
Die Lowlevel-C-Aufrufe reichen hier keine Abortcallbacks durch (3920–3940).
Das bestehende `full()` danach nochmals auszuführen amortisiert diese Arbeit
nicht automatisch und kann die Abbruchlatenz erhöhen.

Ein `encode(0)` betrachtet nur das erste Modell-Audiofenster (normal 30 s),
nicht beliebig die gesamten 300 s einer Aufnahme. C++ begrenzt die gelesenen
Mel-Frames auf `mel_offset + 2*n_audio_ctx` (2383–2394). Ein globaler
No-Speech-Wert aus dem ersten Fenster könnte daher spätere Sprache fälschlich
verwerfen. Lange Aufnahmen benötigen separat definierte Fenster-/Offsetregeln
und dürfen nicht pauschal anhand des ersten SOT-Probes verworfen werden.
Ein zusätzlicher vollständiger Encoder pro Fenster kann einen wesentlichen
Laufzeitanteil verdoppeln; genaue Kosten und Cancelgrenzen bleiben offen.

**Beweisgrenze:** Der Audit beweist den Zeilenfehler und bewertet API-Wege aus
Quelltext. Er ersetzt weder einen Red/Green-Nachweis des späteren Patches noch
erneute Kalibrierung, Laufzeitmessung oder die reale P05-Abnahme.

## Ergänzender lokaler Diagnosebeleg vor dem Patch

Der integrierende Auftrag führte nach dem Quellenaudit den unveränderten
42-Fälle-Kalibrierungssatz mit dem Referenzmodell und ausschließlich numerischer
`[DEBUG-nsp]`-Instrumentierung aus. Die 36 Sprachsegmente lagen zwischen
0,00001926601 und 0,000019293406, die vier ausgegebenen Rausch-/Bürosegmente
zwischen 9,329729e-11 und 3,5604897e-10. Die zwei Nullsignal-Fälle erreichten
den Decoder nicht. Diese Zahlen dienen der Fehlerdiagnose, nicht als
Sprachschwellen oder inverse Klassifikation.

Die temporären Engine-/Probe-SHAs waren
`d0e56256e2f919909f1ef50e32c7bfd4d59e38adf016c8d92e5039b5267dd36c` und
`a43c893319050278ddaa0fca262c3f7109ad0edaa108535dffb4d2d0e057b0b3`.
Die Instrumentierung ist entfernt. Das lokale reine Zahlenlog
`artifacts/corpus/diagnostic-nsp-reference-v2.log` hat SHA256
`e1c20fa8e8e6d352bd9b0a814fac5271652dc3799012e8e8640a30c7cddcf46c`.
