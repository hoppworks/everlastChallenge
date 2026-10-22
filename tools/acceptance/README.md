# Historischer Adapter für native Textübergabe

`transport-v3.patch` und `transport-v3.json` bewahren einen isolierten
Testadapter für die Übergabe einer festen Textfixture. Basis ist ausschließlich
Commit `d2546c438b8f5079f52b5415c181a0fe9129d98b`. Der Patch gehört nicht zum
normalen App-Build und ist nicht gegen den aktuellen Hauptzweig anzuwenden.

Der damalige Build und die Quellenprüfung sind im Begleit-JSON gebunden. Ein
Build allein beweist weder einen ausgeführten Hotkey noch beobachteten Empfang
in einem fremden Textfeld. Der Adapter enthält keine Mikrofonaufnahme oder
Modellinferenz; vorhandene Systemberechtigungen der Produkt-App werden nicht
auf seine eigene Bundle-ID übertragen.

Für eine historische Reproduktion müssen in einem separaten Checkout zuerst
Basis-, Patch- und Ergebnisdatei-Hashes aus `transport-v3.json` übereinstimmen.
Danach lautet der damalige Buildbefehl:

```sh
cargo build --locked --offline -p everlast-dictation --features measurement --example transport_acceptance
```

Aktuelle Produktbedienung und Prüfungen stehen im [README](../../README.md)
und im [Testleitfaden](../../docs/autonomous-testing.md). Die dort beschriebenen
Tests unterscheiden weiterhin zwischen Versand eines Paste-Ereignisses und
tatsächlich beobachtetem Textempfang.
