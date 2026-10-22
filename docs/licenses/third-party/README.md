# Drittanbieter-Lizenzen und Herkunft

Der aktuelle Stand vom 22.10.2026 steht im
[Lockfile-gebundenen Inventar](refinement-dependencies-2026-10-22.json):
398 native Paketkandidaten für Desktop und Text-Hilfsprozess und sechs npm-Bestandteile einschließlich der
Werkzeuge mit eingebrachtem Laufzeithelfer. Das Inventar enthält Versionen,
Quellen und Verweise auf die unter `texts/` erhaltenen Originaltexte.

Die native Liste ist ein konservativer aufgelöster Paketgraph. Sie beweist
nicht, welche Teile jedes Pakets tatsächlich in das fertige Binary gelinkt
wurden. Gesammelte Lizenztexte allein belegen auch keine vollständige
rechtliche Prüfung aller Bestandteile.

Die Grammatikbegrenzung des experimentellen Textworkers verwendet llguidance
1.9.1 und dessen Tokenizeradapter. `borrow-or-share 0.2.4` verwendet MIT-0;
Originaltext und Paketquelle sind im Inventar erhalten. Die Lizenzfreigabe ist
in `deny.toml` auf genau dieses Paket und diese Version beschränkt.

## Ergänzende Herkunftsnachweise

- [Modellbereitstellung](../MODELS.md): Modellgewichte und Code besitzen
  getrennte Herkunftshinweise; Gewichte sind nicht Teil des App-Downloads.
- [Neue Pakettexte](package-update-supplement.json): Ergänzungen für den
  aktualisierten Abhängigkeitsstand.
- [Lizenznachtrag](license-completion.json): FP16-Originaltext, ergänzte
  objc2-Texte und der dokumentierte realfft-Quellenbefund.
- [objc2-Vorgängerhinweise](objc2-predecessor-notices.json): separat erhaltene
  Originalhinweise der Vorgängerprojekte.
- [Native MLX-Herkunft](mlx-native-provenance.json): Quellen, Archive und
  verbleibende Attributionsgrenzen der nativen Bestandteile.
- Lokale Änderungen an übernommenem Code stehen in den jeweiligen
  `vendor/*/PATCHES.md` beziehungsweise `EVERLAST-PATCH.md` samt Quellenbindung.

## Verbleibende Quellen- und Nachweisgrenzen

**realfft 3.5.0** deklariert MIT, enthält im verwendeten veröffentlichten Paket
und gepinnten Originalbaum jedoch keine vollständige originale Lizenzdatei mit
Copyrightnotice. Lizenz- und Autorenmetadaten bleiben unverändert erhalten.
Der separat bezeichnete standardisierte MIT-Permission-/Disclaimer-Text ist
keine angebliche Upstreamdatei; ein Inhaber oder Jahr wurde nicht erfunden.
Diese Grenze betrifft den tatsächlich verwendeten FFT-Resampler.
Die [Quellenprüfung](../../verification/license-completion-2026-10-21.md)
beschreibt die Recherche und ihre Grenzen.

Der genaue Umfang statisch eingebundener Teile und daraus folgender
NOTICE-/Quellhinweise ist durch die Paketliste allein nicht bestimmt. Die
[Bundle-Prüfung](../../verification/release-0.2.0.md) bestätigt Paketdateien und
Signaturen, keine vollständige juristische Freigabe oder exakte Linkanalyse.

## Historische Inventare

[components.json](components.json), [mlx-components.json](mlx-components.json)
und [sha256-acceleration.json](sha256-acceleration.json) bewahren frühere
Quellen- und Lizenzzuordnungen. Ihre Paketstände sind nicht als aktuelle
Versionsliste zu lesen. Insbesondere verwendet der aktuelle eigene Hashpfad
`sha2 0.11.1` ohne die frühere direkte `sha2-asm`-Abhängigkeit.
Die damaligen Originaltexte bleiben zur Nachvollziehbarkeit erhalten.
