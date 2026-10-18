# Drittbestandteile: vorbereitete Lizenztexte

Stand **b34914c7aa88402208850fa589f5c2ee17f07705**, Ziel Apple Silicon/macOS.
**Vorbereitet; finale Linkclosure und Release-Bundle noch nicht geprüft.**

Die Sammlung enthält unveränderte Originaltexte für vier produktive npm-Pakete
und eine konservative Vereinigung von 311 nativen Normaldependency-Kandidaten.
Optionale Featurevereinigung und Proc-Macro-/Generierungsanteile sind nicht
mit tatsächlich gelinktem Runtimecode gleichzusetzen. Reine Build-/Dev-Abschnitte
und fremde Targetabschnitte wurden soweit statisch bestimmbar ausgeschlossen.
Unaufgelöste cfg-Ausdrücke bleiben im Inventar sichtbar und konservativ enthalten.

Die vollständige Zuordnung von Komponente, Version, Herkunft, Paketchecksumme,
Originaldatei, Lizenzalternative und unverändertem Text steht in
[components.json](components.json). Identische Texte werden durch ihren
SHA-256-Dateinamen in texts/ gemeinsam verwendet; Copyright-/NOTICE-Inhalte
werden nicht entfernt. Das Beifügen mehrerer angebotener Lizenztexte ändert
weder deren ursprüngliche Bedingungen noch die Lizenz der Anwendung.

## Offene, konkrete Vollständigkeitsgrenze

- realfft 3.5.0: deklarierte MIT-Lizenz, jedoch kein Lizenzvolltext oder
  Copyrightnotice im veröffentlichten Paket bzw. gepinnten Originalbaum gefunden.
  Die separat bezeichnete standardisierte MIT-Permission-/Disclaimer-Ergänzung
  ist keine vorgebliche Upstreamdatei; sie enthält keinen erfundenen Inhaber,
  kein Jahr und keinen Platzhalter. Die fehlende Originalnotice bleibt als
  Quellenbefund sichtbar, ohne daraus eine eigenständige juristische Freigabe
  oder Sperrentscheidung abzuleiten. Der produktive rubato-
  FFT-Resampler verwendet diese Bibliothek; es ist nicht nur ein Zufallsfund.
- Der endgültige arm64-Target-/Feature- und Bundleabgleich muss klären, welche
  konservativen Kandidaten oder Teile tatsächlich weitergegeben werden, und
  ob deren erforderliche Lizenz-/NOTICE-/Quellhinweise vollständig beiliegen.
  Die Sammlung behauptet keine bereits abgeschlossene P11-Abnahme.

Für MPL-2.0-Kandidaten werden Originaltexte beziehungsweise der unveränderte
offizielle Lizenztext und exakte Paketquellen/Checksummen angegeben. Welche
Teile im finalen Artefakt liegen und welche begleitenden Quellhinweise dort
einschlägig sind, bleibt beim finalen Target-/Bundleabgleich zu bestätigen.
Die Versionsquellen sind über die im Inventar angegebenen crates.io-Paket-URLs
erreichbar. Eigene Änderungen an venderten Paketen stehen weiterhin in deren
PATCHES.md; Originalarchive und Patchherkunft bleiben getrennt.

## Komponentenverzeichnis

| Komponente | Version | ursprüngliche Lizenzangabe | Einordnung / Textstatus |
|---|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| aho-corasick | 1.1.5 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause | konservativer nativer Kandidat; provided |
| alloc-stdlib | 0.3.0 | BSD-3-Clause | konservativer nativer Kandidat; provided |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| base64 | 0.21.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| base64 | 0.22.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| base64 | 0.23.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| bit-set | 0.8.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| bitflags | 1.3.2 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| block2 | 0.6.2 | MIT | konservativer nativer Kandidat; provided |
| brotli | 9.0.0 | BSD-3-Clause AND MIT | konservativer nativer Kandidat; provided |
| brotli-decompressor | 6.0.1 | BSD-3-Clause/MIT | konservativer nativer Kandidat; provided |
| bs58 | 0.5.1 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| byteorder | 1.5.0 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| bytes | 1.12.1 | MIT | konservativer nativer Kandidat; provided |
| camino | 1.2.6 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| cargo_metadata | 0.19.2 | MIT | konservativer nativer Kandidat; provided |
| cfb | 0.14.0 | MIT | konservativer nativer Kandidat; provided |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| chrono | 0.4.45 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| cookie | 0.18.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| core-graphics | 0.25.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| core-graphics-types | 0.2.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| coreaudio-rs | 0.11.3 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| coreaudio-sys | 0.2.18 | MIT | konservativer nativer Kandidat; provided |
| cpal | 0.15.3 | Apache-2.0 | konservativer nativer Kandidat; provided |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| cssparser | 0.37.0 | MPL-2.0 | konservativer nativer Kandidat; provided |
| cssparser-macros | 0.7.1 | MPL-2.0 | konservativer nativer Kandidat; provided |
| ctor | 1.0.13 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| darling | 0.23.0 | MIT | konservativer nativer Kandidat; provided |
| darling_core | 0.23.0 | MIT | konservativer nativer Kandidat; provided |
| darling_macro | 0.23.0 | MIT | konservativer nativer Kandidat; provided |
| dasp_sample | 0.11.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| defmt | 1.2.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| defmt-macros | 1.2.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| defmt-parser | 1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| deranged | 0.5.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| derive_more | 2.1.1 | MIT | konservativer nativer Kandidat; provided |
| derive_more-impl | 2.1.1 | MIT | konservativer nativer Kandidat; provided |
| digest | 0.10.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dirs | 7.0.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dom_query | 0.28.0 | MIT | konservativer nativer Kandidat; provided |
| dpi | 0.1.2 | Apache-2.0 AND MIT | konservativer nativer Kandidat; provided |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dtoa-short | 0.3.5 | MPL-2.0 | konservativer nativer Kandidat; provided |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | konservativer nativer Kandidat; provided |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| embed_plist | 1.2.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| fnv | 1.0.7 | Apache-2.0 / MIT | konservativer nativer Kandidat; provided |
| foldhash | 0.2.0 | Zlib | konservativer nativer Kandidat; provided |
| foreign-types | 0.5.0 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| foreign-types-macros | 0.2.4 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| foreign-types-shared | 0.3.1 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| generic-array | 0.14.7 | MIT | konservativer nativer Kandidat; provided |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| glob | 0.3.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| global-hotkey | 0.8.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| heck | 0.5.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| hex | 0.4.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| html5ever | 0.39.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| http | 1.5.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| http-body | 1.1.0 | MIT | konservativer nativer Kandidat; provided |
| http-body-util | 0.1.5 | MIT | konservativer nativer Kandidat; provided |
| httparse | 1.10.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| hyper | 1.12.0 | MIT | konservativer nativer Kandidat; provided |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | konservativer nativer Kandidat; provided |
| hyper-util | 0.1.21 | MIT | konservativer nativer Kandidat; provided |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| ico | 0.5.0 | MIT | konservativer nativer Kandidat; provided |
| icu_collections | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_locale_core | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_normalizer | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_properties | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_properties_data | 2.3.0 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| icu_provider | 2.3.1 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| ident_case | 1.0.1 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| idna | 1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| image | 0.25.10 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| infer | 0.22.0 | MIT | konservativer nativer Kandidat; provided |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| itoa | 1.0.18 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| jiff | 0.2.38 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| jiff-core | 0.1.1 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| jiff-static | 0.2.38 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| json-patch | 4.2.0 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| keyboard-types | 0.7.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| libc | 0.2.190 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| litemap | 0.8.3 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| log | 0.4.34 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| lru-slab | 0.1.3 | MIT OR Apache-2.0 OR Zlib | konservativer nativer Kandidat; provided |
| mach2 | 0.4.3 | BSD-2-Clause OR MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| memchr | 2.8.3 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| mime | 0.3.17 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | konservativer nativer Kandidat; provided |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | konservativer nativer Kandidat; provided |
| mio | 1.2.4 | MIT | konservativer nativer Kandidat; provided |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | konservativer nativer Kandidat; provided |
| muda | 0.20.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| new_debug_unreachable | 1.0.6 | MIT | konservativer nativer Kandidat; provided |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| objc2 | 0.6.5 | MIT | konservativer nativer Kandidat; provided |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-av-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-cloud-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-data | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-graphics | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-image | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-text | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-core-video | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-encode | 4.1.0 | MIT | konservativer nativer Kandidat; provided |
| objc2-exception-helper | 0.1.1 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-foundation | 0.3.2 | MIT | konservativer nativer Kandidat; provided |
| objc2-io-surface | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-quartz-core | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| objc2-web-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| option-ext | 0.2.0 | MPL-2.0 | konservativer nativer Kandidat; provided |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| phf | 0.13.1 | MIT | konservativer nativer Kandidat; provided |
| phf_generator | 0.13.1 | MIT | konservativer nativer Kandidat; provided |
| phf_macros | 0.13.1 | MIT | konservativer nativer Kandidat; provided |
| phf_shared | 0.13.1 | MIT | konservativer nativer Kandidat; provided |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| plist | 1.10.1 | MIT | konservativer nativer Kandidat; provided |
| png | 0.17.16 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| png | 0.18.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| potential_utf | 0.1.6 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| precomputed-hash | 0.1.1 | MIT | konservativer nativer Kandidat; provided |
| primal-check | 0.3.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | konservativer nativer Kandidat; provided |
| quick-xml | 0.42.0 | MIT | konservativer nativer Kandidat; provided |
| quinn | 0.11.12 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| quinn-proto | 0.11.19 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| quinn-udp | 0.5.16 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| quote | 1.0.47 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rand | 0.10.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | konservativer nativer Kandidat; provided |
| realfft | 3.5.0 | MIT | produktive FFT-Abhängigkeit; standardisierte Ergänzung, Originalnotice unbekannt |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| regex | 1.13.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| ring | 0.17.14 | Apache-2.0 AND ISC | konservativer nativer Kandidat; provided |
| rtrb | 0.3.5 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rubato | 0.16.2 | MIT | konservativer nativer Kandidat; provided |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| rustfft | 6.4.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | konservativer nativer Kandidat; provided |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| rustls-webpki | 0.103.15 | ISC | konservativer nativer Kandidat; provided |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | konservativer nativer Kandidat; provided |
| same-file | 1.0.6 | Unlicense/MIT | konservativer nativer Kandidat; provided |
| schemars | 0.8.22 | MIT | konservativer nativer Kandidat; provided |
| schemars | 0.9.0 | MIT | konservativer nativer Kandidat; provided |
| schemars | 1.2.3 | MIT | konservativer nativer Kandidat; provided |
| schemars_derive | 0.8.22 | MIT | konservativer nativer Kandidat; provided |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| selectors | 0.38.0 | MPL-2.0 | konservativer nativer Kandidat; provided |
| semver | 1.0.28 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde | 1.0.228 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_core | 1.0.228 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_derive | 1.0.228 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_json | 1.0.149 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_with | 3.22.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serde_with_macros | 3.22.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| simd-adler32 | 0.3.10 | MIT | konservativer nativer Kandidat; provided |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| slab | 0.4.12 | MIT | konservativer nativer Kandidat; provided |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| strength_reduce | 0.2.4 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| string_cache | 0.9.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| strsim | 0.11.1 | MIT | konservativer nativer Kandidat; provided |
| subtle | 2.6.1 | BSD-3-Clause | konservativer nativer Kandidat; provided |
| swift-rs | 1.0.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| syn | 2.0.119 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| syn | 3.0.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| sync_wrapper | 1.0.2 | Apache-2.0 | konservativer nativer Kandidat; provided |
| synstructure | 0.14.0 | MIT | konservativer nativer Kandidat; provided |
| tao | 0.37.1 | Apache-2.0 | konservativer nativer Kandidat; provided |
| tauri | 2.12.4 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-codegen | 2.7.3 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-macros | 2.7.3 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-plugin-global-shortcut | 2.4.1 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-runtime | 2.12.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-runtime-wry | 2.12.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tauri-utils | 2.10.2 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tendril | 0.5.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| termcolor | 1.4.1 | Unlicense OR MIT | konservativer nativer Kandidat; provided |
| thiserror | 2.0.18 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| thiserror-impl | 2.0.18 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| time | 0.3.55 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| time-core | 0.1.9 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| tinystr | 0.8.4 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| tokio | 1.53.3 | MIT | konservativer nativer Kandidat; provided |
| tokio-macros | 2.7.2 | MIT | konservativer nativer Kandidat; provided |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| tokio-util | 0.7.20 | MIT | konservativer nativer Kandidat; provided |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| tower | 0.5.3 | MIT | konservativer nativer Kandidat; provided |
| tower-http | 0.6.11 | MIT | konservativer nativer Kandidat; provided |
| tower-layer | 0.3.3 | MIT | konservativer nativer Kandidat; provided |
| tower-service | 0.3.3 | MIT | konservativer nativer Kandidat; provided |
| tracing | 0.1.44 | MIT | konservativer nativer Kandidat; provided |
| tracing-core | 0.1.36 | MIT | konservativer nativer Kandidat; provided |
| transpose | 0.2.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| tray-icon | 0.25.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| try-lock | 0.2.5 | MIT | konservativer nativer Kandidat; provided |
| ts-rs | 11.1.0 | MIT | konservativer nativer Kandidat; provided |
| ts-rs-macros | 11.1.0 | MIT | konservativer nativer Kandidat; provided |
| typeid | 1.0.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| typenum | 1.20.1 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | konservativer nativer Kandidat; provided |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| untrusted | 0.9.0 | ISC | konservativer nativer Kandidat; provided |
| url | 2.5.8 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| urlpattern | 0.6.0 | MIT | konservativer nativer Kandidat; provided |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| uuid | 1.28.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| walkdir | 2.5.0 | Unlicense/MIT | konservativer nativer Kandidat; provided |
| want | 0.3.2 | MIT | konservativer nativer Kandidat; provided |
| web-time | 1.1.0 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 | konservativer nativer Kandidat; provided |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 | konservativer nativer Kandidat; provided |
| whisper-rs | 0.16.0 | Unlicense | konservativer nativer Kandidat; provided |
| whisper-rs-sys | 0.15.0 | Unlicense | konservativer nativer Kandidat; provided |
| window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| winnow | 1.0.4 | MIT | konservativer nativer Kandidat; provided |
| writeable | 0.6.4 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| wry | 0.57.0 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| yoke | 0.8.3 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| yoke-derive | 0.8.4 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zerofrom | 0.1.8 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | konservativer nativer Kandidat; provided |
| zerotrie | 0.2.5 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zerovec | 0.11.8 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zerovec-derive | 0.11.6 | Unicode-3.0 | konservativer nativer Kandidat; provided |
| zlib-rs | 0.6.8 | Zlib | konservativer nativer Kandidat; provided |
| zmij | 1.0.23 | MIT | konservativer nativer Kandidat; provided |
| react | 19.3.1 | MIT | JS-Produktabhängigkeit; provided |
| react-dom | 19.3.1 | MIT | JS-Produktabhängigkeit; provided |
| scheduler | 0.28.0 | MIT | JS-Produktabhängigkeit; provided |
| @tauri-apps/api | 2.12.4 | Apache-2.0 OR MIT | JS-Produktabhängigkeit; provided |
