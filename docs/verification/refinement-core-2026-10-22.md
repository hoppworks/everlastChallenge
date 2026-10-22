# Deterministische Textprüfung

Die Bibliothek `dictation-core::refinement` ergänzt eine reine Rust-Grenze für
lokale Textbearbeitung. Sie lädt kein Modell und ist noch nicht in den aktuellen
Diktierablauf eingebunden.

## Geprüfte Verträge

- Das Original bleibt unverändert; nur ein vollständig gültiger Änderungssatz
  ergibt einen separat akzeptierten Text.
- Änderungen benötigen eindeutige, zusammenhängende Originalspannen. Optionaler
  direkter Kontext löst Wiederholungen auf. Überlappende und unvollständige
  Verankerungen werden zurückgewiesen.
- Fülllaute, explizite Selbstkorrekturen und begrenzte Satzzeichen-/Satzanfangs-
  korrekturen werden getrennt geprüft. Literale, Zitate, Zahlenbestandteile und
  sinntragende Großschreibung dürfen nicht beiläufig verändert werden.
- Kommas bei Fülllauten müssen ausdrücklich von der Quellspanne erfasst werden;
  ein vorhandenes Komma kann im Ersatz erhalten bleiben. Leerzeichen werden nur
  an der geprüften Löschgrenze behandelt.
- Der Bedarf wird für Deutsch und Englisch bestimmt. Kurze Selbstkorrekturen
  werden nicht pauschal übersprungen. Zitat-/Literalinhalte lösen für sich allein
  keine Füllwortkorrektur aus.
- Eingabe, Anzahl der Änderungen, Kontext und Ersetzungen besitzen feste Grenzen.

Die Tests verwenden fest vorgegebene erwartete Texte und Gegenbeispiele über die
öffentlichen Funktionen `decide` und `validate`. Sie rufen keine internen Helfer auf.

## Unabhängige Review und Regressionen

Eine unabhängige kombinierte Code-Review fand vier konkrete Lücken im ersten
Stand: Füllwortlöschung innerhalb eines Links, Teilersetzung einer Dezimalzahl,
eine unvollständig erfasste Komma-Spanne und die Ablehnung einer eindeutigen
Selbstkorrektur ohne bereits vorhandenes Komma. Die neuen Regressionen scheiterten
vor der Korrektur und bestehen mit der gemeinsamen Literalprüfung, vollständigen
Spannenkontrolle und angepassten Markererkennung. Der Literalschutz wird auch bei
der Bedarfsentscheidung berücksichtigt; er darf Zahlkorrekturen nicht verdecken.

Die anschließende Review der geänderten Markererkennung fand einen weiteren
Fall: gewöhnliches „nein/no“ wurde zu weit als Korrektur ausgelegt. Eigene
Gegenbeispiele für Pfade und normale Verneinung decken dies nun ab. Schwache
Marker benötigen erkennbare parallele Werte oder passende, ausdrücklich
abgetrennte Namensphrasen; technische Binnenmarker sind ausgeschlossen.

## Grenzen

Die Tests prüfen deterministische Regeln, keine allgemeine Bedeutungsäquivalenz.
Eine erkannte Bedarfsentscheidung ist keine sprachliche Qualitätsgarantie.
Reale Modellqualität, Parser-/Prozessprotokoll, Abbruch der Inferenz, Oberfläche,
native Textübergabe, Speicherbedarf und Produktperformance sind hier nicht
geprüft. Dafür bleiben eigene Integrations- und Laufzeitnachweise erforderlich.

Reproduktion: `cargo test -p dictation-core --locked` und
`cargo clippy -p dictation-core --all-targets --locked -- -D warnings`.
