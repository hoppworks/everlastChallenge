# Isolierte Exact-State-Diagnose V2

Dieses Quellpaket ergaenzt den bestehenden Engineprofiler um Rohzaehler des
tatsaechlich verwendeten WhisperState. Es ist ein separat gebautes Messderivat,
keine Aenderung am Produktbaum. Basis ist exakt
`684343309d6e0536215bcd448a604ae7ea676d4e` plus
`exact-state-counter-v2.patch`; alle 19 Ergebnisse stehen in der JSON-Datei.
Der gepruefte lokale Mac-Quellencommit des Derivats ist
`28d85762a71c2723e600889338c2bd7adfa24c6e`.

## Reproduktion und Featuregrenze

Einen frischen separaten Checkout der Basis erstellen, Basis-/Patchhash und
alle Zielbindungen pruefen, dann den Patch nur dort anwenden. Fuer einen
unveraenderten Snapshot ein frisches eigenes CARGO_TARGET_DIR verwenden.
Zuerst den bestehenden engine_profile_probe mit `--release --locked --offline`
und Feature `measurement` bauen; Binary und Parameterausgabe konservieren.
Danach denselben Snapshot mit `state-perf-diagnostic` bauen. Kompatible
Abhaengigkeiten duerfen wiederverwendet werden; die beiden sys-Features muessen
verschiedene frische OUT_DIRs erhalten. Native Header/C++ innerhalb eines
bereits benutzten Ausgabeordners nicht nachtraeglich aendern.

Der normale native Build besitzt kein Gettersymbol. Nur das Diagnosefeature
uebertraegt dasselbe Makro an Bindgen und C++; die Diagnose lehnt gebuendelte
Fallbackbindings ab. Der Getter kopiert sechs rohe i64-Mikrosekundensummen
und sieben i32-Zaehler in einen festen 80-Byte-POD. Der sichere Rustzugriff
benoetigt einen exklusiven WhisperState. Kein Reset, keine Allokation, keine
Timerabfrage, kein Kontext-State-Ersatz und keine nebenlaeufige Abfrage.

Der Collector liest ausschliesslich nach erfolgreichem unveraendertem full(),
vor Textjoin und Drop. Bei Native-Fehler werden Getter und dessen Uhr nicht
aufgerufen: Rohwerte und Lesedauer bleiben null. Die bekannte vorgelagerte
Native-Full-/Drop-Lebensdauergrenze wird dadurch nicht repariert. Parameter,
Modell, VAD, Decoder, Fallback/RNG und Statebesitz bleiben unveraendert.

## Tatsaechliche Belege

Der kombinierte Source-/ABI-/Build-/Ownershipreview ist abgeschlossen; der
fehlende optionale Initializer im kontrollierten Example-Testprofil wurde in
V2 korrigiert. Sieben portable Tests und kontrollierte Counter-/Fehlerpfad-
Mutanten stammen vom oeffentlichen Code-only-Linuxlauf mit Rust 1.97/GCC 16. Diese
Belege ersetzen keinen Mac-Build. Die bestehende Getter-/ABI-Testlogik wurde
wegen des engen Testkorrekturdeltas nicht unnoetig wiederholt.

Auf dem Mac mit Rust 1.93 bestehen beide Releasebuilds, zwoelf integrierte
Example-Tests, Strict Clippy im Testprofil und die C11-ABI-Kompilation. Die
Symbolgrenze, generierten Bindings, kopierten C++-Inputs und getrennten
nativen Ausgabeverzeichnisse sind tatsaechlich geprueft. Der unabhaengige
Abschlussreadback verifiziert alle 1578 Quellen gegen den unveraenderlichen
Derivatcommit und alle Build-/Eingabe-/Prozess-/Request-/Worker-/Joinbelege.

Eine prospektiv gebundene DE-Kalibrieraufnahme mit 144000 Frames wurde genau
viermal aufgerufen: erster Aufruf warm, drei Messungen. NativeFull liegt
deskriptiv bei 1,000165 bis 1,000309 Sekunden; rohe Encode-Summen bei 901618
bis 901994 Mikrosekunden. Alle vier Texthashes stimmen mit dem vorherigen
Referenzprofil derselben Aufnahme ueberein. Keine Hypothese wird hier
veroeffentlicht. encodeUs umfasst Conv, Encoder und Cross-Projektion;
Native-Zaehler sind keine GPU-Auslastung oder vollstaendige Zeitpartition.
Signierte Residuen und rohe Counts bleiben erhalten. Getterwerte von 0/0/125
Nanosekunden sind begrenzte Uhraufloesung, kein Null-Overhead-Nachweis.

## Grenzen

Das Derivat aendert drei historische Qualitaetspins; 44 bleiben bytegleich.
Eine alte 47-Quellen-Freigabe wird nicht auf die Instrumentierung umetikettiert.
Die einzelne Kalibrieraufnahme beweist weder Corpusqualitaet noch L, p95 oder
Ressourcenabnahme. Das Produkt bleibt unveraendert. Ein kleinerer Audiokontext
ist ein getrennter zukuenftiger Versuch, kein Ergebnis dieses Pakets.

Die Patchbytes bleiben original, einschliesslich normaler Leerzeichen auf
leeren Unified-Diff-Kontextzeilen. Die angewandten Quellen wurden separat
formatiert/geprueft. Nicht aus dem dokumentierten lokalen Binaryhash auf einen
spaeteren Neubuild schliessen: Quelle, Toolchain, Features und neue tatsaechliche
Binardatei muessen fuer jeden ausgefuehrten Nachweis gebunden werden.
