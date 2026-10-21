# Referenzanker-Inventar v1 – vor Rohtextprüfung festgeschrieben

**Status: NACH bereits vorhandener ASR erstellt; ausschließlich unveränderte Publisher-Referenzen gelesen.** Keine Hypothesen, ASR-Ausgaben oder Audios wurden für dieses Inventar eingesehen. Dies ist weder Originalhörprüfung noch Genauigkeitsabnahme und keine nachträgliche Änderung an Referenzen, Decoderparametern oder WER-Normalisierung. Root hat die vollständige Liste vor einer Hypothesenansicht gelesen und die unten festgehaltene Bewertung beschlossen. Ein unabhängiger Readback bestätigte vor der Rohtextprüfung alle 75 Fälle, 158 wörtlichen Spans und das Verfahren ohne Befund.

Quelle: `tests/fixtures/corpus/acceptance-v1.json`, SHA-256 `7d7a25695ab77d9d4affe04b0bbc1ce5f8bc4b38a59b40763fd26884d090c67d`. Vollständiger beauftragter Teilbestand: **75 Fälle**, davon 41 DE-/30 EN-clean-FLEURS und vier clean digit-anchor acceptance cases (DE acht/neun, EN eight/nine). Die übrigen 15 Acceptancefälle sind nicht Teil dieses Inventarauftrags. Die Referenzprovenienz bleibt die des Manifests: veröffentlichter FLEURS-Text bzw. unabhängiges Digit-Klassenlabel; keine neue manuelle Transkription.

## Kategorien und Grenzen

- **N**: explizite Negation einschließlich „niemand/no one“, „cannot“ und „unable“. Die Originalform bleibt erhalten; Negation darf nicht aus dem Kontext isoliert als Bedeutungsprüfung behandelt werden (beispielsweise verneinte Aufforderung „Don't forget“).
- **Z**: konkrete Zahlen-/Maßaussagen, Jahreszahlen, Brüche, Multiplikatoren und numerischer Modellcode M16. Einheiten, Vergleich „unter“ und zusammengehörige Bruch-/Dimensionsangaben bleiben im Span. „beiden“ ist die definite Zweizahl. „eine Minute/a minute“ bezeichnet eine konkrete Dauer und ist deshalb aufgenommen.
- **E**: Personen, Orte, Organisationen, Werktitel, benannte historische Ereignisse, astronomischer Eigenname Erde/Earth und kulturelle Bezeichnung Romantik/Romanticism. Ethnonyme Juden/Jews und Palästinenser sind ausdrücklich mit inventarisiert, ihre Einordnung als klassische Eigennamen ist prüfbedürftig.
- **?**: zweifelhafte Grenzfälle separat, keine stillschweigend verpflichtenden Anker. Dies umfasst lexikalische/semantische Verneinung (abgesagt/cancelled, verboten, non-/Nicht-, anti-, unabhängig/independent, eliminate, anstatt), ordinale/partitive Formen, übersetzte Institutionen, generische geografische/institutionelle Bezeichnungen und Speisenbezeichnung.
- Reine Artikel wie „ein Ball/a ball“ und Pronomen wie „for one“ sind keine sicheren Zahlbehauptungen. Unbestimmte Mengen „viele/many“, „mehrere/several“, „einige/some“, „mehrfach/multiple“ sowie Exklusiv-/Gesamtheitsquantoren „nur/only“, „alle/all“ sind keine expliziten Cardinalzahlen und hier nicht als Z erfasst. „Geben Sie acht“ ist **keine Zahl acht**. Wortbestandteile von „Sechstagekrieg“, „Eintagsfliegen“ oder „mayflies“ werden nicht als eigenständige Zahlen aufgespalten.
- Jede Tabellenzeile bezeichnet genau einen Fall. „—“ bedeutet: im beauftragten Kategorienumfang kein expliziter Anker gefunden; der Fall wurde dennoch durchgesehen. Alle aufgeführten Spans wurden zusätzlich gegen den jeweiligen unveränderten Referenzstring auf wörtliches Vorkommen geprüft. Keine automatische Aussage über Aussprache, Zeitgrenzen, Hörbarkeit oder korrekte Hypothese.

## Vollständige fallweise Liste

| Fall-ID | Sprache | N: Originalspan | Z: Originalspan | E: Originalspan | ?: Originalspan / Prüffall |
|---|---|---|---|---|---|
| fleurs-de-test-1660 | de | — | — | „Romantik“; „Goethe“; „Fichte“; „Schlegel“ | — |
| fleurs-de-test-1661 | de | „keine“ | — | „Chinas“ | — |
| fleurs-de-test-1662 | de | „nicht“ | „zwei oder mehr“ | — | — |
| fleurs-de-test-1663 | de | — | — | „Cochamó-Tal“; „Chiles“; „Yosemite-Nationalpark“; „Südamerika“ | — |
| fleurs-de-test-1664 | de | — | „zwei“ | — | — |
| fleurs-de-test-1665 | de | — | „1967“ | „Palästinenser“; „Sechstagekrieg“ | — |
| fleurs-de-test-1666 | de | — | — | — | — |
| fleurs-de-test-1667 | de | — | „zwei“ | — | „Das eine Ende“; „einer der Jungs“; „eines der Mädels“ |
| fleurs-de-test-1668 | de | — | — | „Wild Card“; „Südafrika“ | „südafrikanischen Nationalparks“ |
| fleurs-de-test-1669 | de | — | — | „Chandra Shekhar Solanki“ | — |
| fleurs-de-test-1670 | de | „nicht“ | — | „Erde“; „Glen Cushing“; „United States Geological Survey“; „USGS“; „Northern Arizona University“; „Flagstaff“; „Arizona“ | „Astrogeologie-Team“ |
| fleurs-de-test-1671 | de | — | „2005“; „10“ | „FBI“ | „Kongress“; „anti-Obszönitätsinitiative“ |
| fleurs-de-test-1672 | de | „nicht“ | — | — | „Geben Sie acht“ |
| fleurs-de-test-1673 | de | — | „13“ | — | „Revolutionskämpfe“; „Bundesvertrages“; „Kongress“; „zuerst“ |
| fleurs-de-test-1674 | de | — | „eine Minute“ | — | — |
| fleurs-de-test-1675 | de | — | — | — | — |
| fleurs-de-test-1676 | de | — | — | — | „südafrikanischen Nationalparks“ |
| fleurs-de-test-1677 | de | „nicht“ | — | — | — |
| fleurs-de-test-1678 | de | „keine“ | — | „Die Rede des Präsidenten“; „Oliver Sacks“ | — |
| fleurs-de-test-1679 | de | — | — | „Aerosmith“ | „abgesagt“ |
| fleurs-de-test-1680 | de | „nicht“ | „fünffachen“ | — | — |
| fleurs-de-test-1682 | de | „niemand“ | „29¾ Zoll x 24½ Zoll“ | — | — |
| fleurs-de-test-1683 | de | — | — | „Juden“ | „Nichtjuden“ |
| fleurs-de-test-1684 | de | — | „knapp unter 40.000“ | „Apia“; „Upolu“; „Samoa“ | — |
| fleurs-de-test-1685 | de | — | „35 mm“ | — | — |
| fleurs-de-test-1686 | de | — | — | — | — |
| fleurs-de-test-1687 | de | — | „beiden“ | — | — |
| fleurs-de-test-1688 | de | — | — | — | „Marine“ |
| fleurs-de-test-1689 | de | — | — | „Europa“ | „unabhängige“ |
| fleurs-de-test-1690 | de | — | — | „USOC“ | „anstatt“ |
| fleurs-de-test-1691 | de | — | — | — | — |
| fleurs-de-test-1692 | de | — | — | „Japan“ | „Nuklearbehörde“ |
| fleurs-de-test-1693 | de | — | „36“; „7“ | „Indien“ | — |
| fleurs-de-test-1694 | de | — | — | — | — |
| fleurs-de-test-1695 | de | — | — | — | „Pa amb Oli“ |
| fleurs-de-test-1696 | de | — | „zwei“ | — | — |
| fleurs-de-test-1697 | de | „nicht“ | — | „Commonwealth Games“ | „verboten“ |
| fleurs-de-test-1698 | de | — | — | — | — |
| fleurs-de-test-1699 | de | — | — | „Bischkek“ | — |
| fleurs-de-test-1700 | de | — | — | — | „Mare“ |
| fleurs-de-test-1736 | de | — | „M16“ | „Rolando Mendoza“ | „M16-Gewehr“ |
| fleurs-en-test-1660 | en | — | — | „Romanticism“; „Goethe“; „Fichte“; „Schlegel“ | — |
| fleurs-en-test-1661 | en | „not“ | — | „China's“ | — |
| fleurs-en-test-1662 | en | „Don't“ | „two or more“ | — | — |
| fleurs-en-test-1663 | en | — | — | „Cochamó Valley“; „Chile's“; „Yosemite“; „South America“ | — |
| fleurs-en-test-1664 | en | — | „two“ | — | — |
| fleurs-en-test-1665 | en | — | „1967“ | „Palestine“; „Mideast War“ | — |
| fleurs-en-test-1666 | en | — | — | — | — |
| fleurs-en-test-1667 | en | — | „two“ | — | „One end“; „one of the guys“ |
| fleurs-en-test-1668 | en | — | — | „Wild Card“; „South Africa“; „South African National Parks“ | „for one“ |
| fleurs-en-test-1669 | en | — | — | „Chandra Shekhar Solanki“ | — |
| fleurs-en-test-1670 | en | „not“ | — | „Earth“; „Glen Cushing“; „United States Geological Survey“; „USGS“; „Northern Arizona University“; „Flagstaff“; „Arizona“ | „Astrogeology Team“ |
| fleurs-en-test-1671 | en | — | „2005“; „10“ | „Congress“; „FBI“ | — |
| fleurs-en-test-1672 | en | „not“ | — | — | — |
| fleurs-en-test-1673 | en | — | „thirteen“ | „Revolutionary War“; „Congress“; „Articles of Confederation“ | „first“ |
| fleurs-en-test-1674 | en | — | „a minute“ | — | — |
| fleurs-en-test-1675 | en | — | — | — | — |
| fleurs-en-test-1676 | en | — | — | „South African National Parks“ | — |
| fleurs-en-test-1677 | en | „cannot“ | — | — | — |
| fleurs-en-test-1678 | en | „unable“ | — | „Oliver Sacks“; „The President's Speech“ | „nevertheless“ |
| fleurs-en-test-1679 | en | — | — | „Aerosmith“ | „cancelled“ |
| fleurs-en-test-1680 | en | „not“ | „five times“ | — | — |
| fleurs-en-test-1681 | en | „isn’t“ | „one or more“ | — | „eliminate“ |
| fleurs-en-test-1682 | en | „no one“ | „29¾ inches by 24½ inches“ | — | — |
| fleurs-en-test-1683 | en | — | — | „Jews“ | „non-Jews“ |
| fleurs-en-test-1684 | en | — | „just under 40,000“ | „Apia“; „Samoa“; „Upolu“ | — |
| fleurs-en-test-1685 | en | — | „35mm“ | — | — |
| fleurs-en-test-1686 | en | — | — | — | — |
| fleurs-en-test-1687 | en | — | „two“ | — | — |
| fleurs-en-test-1688 | en | — | — | — | — |
| fleurs-en-test-1689 | en | — | — | „Europe“ | „independent“ |
| hd-de-acceptance-s04-t0-d8 | de | — | „acht“ | — | — |
| hd-de-acceptance-s05-t0-d9 | de | — | „neun“ | — | — |
| hd-en-acceptance-s04-t0-d8 | en | — | „eight“ | — | — |
| hd-en-acceptance-s05-t0-d9 | en | — | „nine“ | — | — |

## Vor der Rohtextprüfung festgelegte Bewertung

1. Zahlen-/Eigennamenanker werden als ganze Aussagen im Originalkontext geprüft: etwa „knapp unter 40.000“, „29¾ Zoll x 24½ Zoll“ und M16 einschließlich Modellidentität. WER allein garantiert diese Inhaltstreue nicht; aus diesem Entwurf folgt keine neue Bestehensschwelle.
2. Alle N-/Z-/E-Spans bleiben in der Prüfung; kein Anker wird nach Ergebnissichtung entfernt. Auch alle ?-Spans werden kontextbezogen geprüft und berichtet. Ihre sprachliche Kategorisierung allein erzeugt keine zusätzliche Bestehensschwelle: Beispielsweise ist „Geben Sie acht“ keine Behauptung der Zahl acht und ein partitiver Artikel keine neue Kardinalzahl.
3. Dieses Manifest enthält nicht den A22-Satz „Vielen Dank fürs Zuschauen“. Dessen gesonderte synthetische Quelle und tatsächliche Erkennung beider Modelle sind in [synthetic-speech.md](synthetic-speech.md) dokumentiert. Das Inventar erweitert diesen Nachweis nicht und erfindet keine Pflicht zu einer zusätzlichen menschlichen Aufnahme.
4. Dieses Dokument sieht keine neue Auswahl aus Audiodaten vor. Es deckt ausschließlich den oben festgelegten vollständigen Teilbestand ab, nicht challenging-Replikate, Kalibrierung oder spätere Zusatzmanifeste.

## Verfahren und unveränderte Grenzen

- Für jeden relevanten Span die **rohe** Hypothese im ganzen Satzkontext mit der unveränderten veröffentlichten Referenz vergleichen. N/Z/E/? erhalten nachvollziehbare Einzelurteile `pass`, `fail` oder `unclear` mit Begründung. Kein Urteil wird aus WER oder bloßer normalisierter Tokenübereinstimmung abgeleitet.
- Zahlen sind nach Wert, Einheit und Vergleich zu prüfen: „40.000“/„40,000“/„vierzigtausend“ können dieselbe Menge bezeichnen; ein anderes Dezimal-/Bruchverhältnis oder verlorenes „unter“ darf nicht durch Normalisierung verschwinden. Schreibvarianten werden nicht pauschal als Bedeutungsfehler gewertet.
- Negation gehört zur konkreten verneinten Aussage; sinngetreue Formulierungen können andere Oberflächenwörter verwenden. Namen werden auf die bezeichnete Entität und erkennbare Verwechslungen geprüft, Orthografie getrennt berichtet. Der Plan verlangt zusätzlich keine erfundene allgemeine Nullfehlergrenze für jede Namensschreibung. Verlorene Negation, falsche Zahl sowie neue Segmentauslassung/-dopplung bleiben die unveränderten harten Ankerkriterien.
- Keine nachträgliche Auswahl nur erfolgreicher Fälle, Referenzreparatur, WER-Normalisierungsänderung oder Decoderkalibrierung anhand dieses Acceptanceoutputs. Fehler und Unklarheiten bleiben erhalten.
- Dies ist ein semantischer Vergleich gegen belegte Publisherreferenzen, keine Behauptung unabhängig gehörten Audios. Der Plan verlangt keinen pauschalen Hördurchgang aller WAVs. Nur tatsächlich unsichere Zuordnungen, Labels oder Alignments brauchen gezielte zusätzliche Quellen-/Originalprüfung. Allgemeine Labels für Raumton beweisen noch keine Sprachfreiheit des genauen Abschnitts; diese getrennte A24-Frage bleibt offen.
- Die bestehenden `challenging`-Fälle, 30-s-Ergänzungen und der konstruierte 5-min-Fall werden hierdurch weder qualifiziert noch aus dem Korpus entfernt. Sie brauchen eigene transparente Inhalts-/Vollständigkeitsbewertung.

Vertragsklärung: IMPLEMENTATION_PLAN.md §13 (historischer oder lokaler Beleg, nicht mitgeliefert), insbesondere unveränderte Rohinhalte zusätzlich zur WER sowie festgelegte Ankerkriterien. Ein unabhängiger rein textlicher Abgleich des tatsächlichen Plans/Originalauftrags bestätigte, dass kein pauschales Hörverfahren vorgegeben ist; unsichere Inhaltsgrundlagen werden dadurch nicht automatisch gültig.
