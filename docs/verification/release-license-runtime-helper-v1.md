# Lizenzmaterial: injizierter Laufzeithelfer

Vorbereitet, keine juristische Freigabe und keine abgeschlossene P11-Abnahme.
Die Ergänzung ist im Testpaket R3 separat beigelegt;
keine installierte App, Signatur oder historische ZIP-Datei wurde verändert. Es wurde kein Build ausgeführt.

Die historische Inventarbasis bleibt
`b34914c7aa88402208850fa589f5c2ee17f07705`. Das Addendum wurde gegen
`2fc213c681fa7a8999332017d8dc2c478b26ea76` vorbereitet. `materialRevision` im
Inventar bindet die neu gelesenen Quellen getrennt von den alten `sourceInputs`.
Die ursprünglichen 315 Komponentenobjekte bleiben exakt erhalten: 311 native
Kandidaten und vier produktive npm-Abhängigkeiten. Hinzu kommen zwei separat klassifizierte npm-Komponenten des Build-injizierten
Laufzeithelfers (Vite und Provider Rolldown), insgesamt 317 Komponenten.

## Konkreter Befund

`src/main.tsx` enthält lediglich CSS-Import und eigenen Starttext. Das tatsächlich
vorliegende `dist/assets/index-Cp78-0dB.js` (767 Byte,
`e4f21590bafb7c90abb9cb68cd6b269eca4a87fabd6e4ed7b3c619166b89c30f`)
enthält zusätzlich einen modulepreload-Polyfill mit `MutationObserver` und `fetch`.
Der gepinnte Vite-Code `node_modules/vite/dist/node/chunks/node.js`, Symbol
`modulePreloadPolyfillPlugin` (ab Zeile 31557), delegiert den gebündelten
Clientpfad an `rolldown/experimental` → `viteModulePreloadPolyfillPlugin`.
Rolldown 1.2.14s `dist/shared/constructors-BDv3zGhp.mjs` wählt
`builtin:vite-module-preload-polyfill`; der Adapter
`normalize-string-or-regex-COVDiCTB.mjs` reicht Builtins an das native Binding.
Diese konkreten Dateien und Rolldowns Lockintegrität stehen im Inventar.
Die veröffentlichte JS-Kette belegt den Provider, nicht die exakte Autorenschaft
jedes nativen Implementierungsteils. Beide vollständigen Originaldateien
`rolldown/LICENSE` und `rolldown/THIRD-PARTY-LICENSE` sind unverändert
SHA-benannt beigefügt, weitere Upstreamhinweise vorsorglich erhalten.

Die Vite-Einordnung als devDependency schließt diesen injizierten Laufzeitanteil
also nicht aus. Dies ist keine Behauptung, alle Vite-Buildabhängigkeiten würden
mitgeliefert; die endgültige Zuordnung tatsächlich enthaltener Anteile bleibt offen.

`package-lock.json` bindet Vite 8.3.4 an
`https://registry.npmjs.org/vite/-/vite-8.3.4.tgz` mit Integrität
`sha512-IftLE8phJhdNRIGNEV4JCCvfNScoZQVfcXaN8TQropWGJtOHcu+iAB9pHcuGvmEb/kddw0LBe/jS8ztdC6xdiQ==`.
Der **vollständige, unveränderte** lokale Originaltext `node_modules/vite/LICENSE.md`
(108786 Byte) wurde unter
`docs/licenses/third-party/texts/a47172e44a86a7e2cd1c16282341f7726ec60caafe3a6166fe3e0602005b6fa6.txt`
übernommen. Er enthält Vites MIT-Text samt weiteren Originalhinweisen; keine
Copyrightzeile wurde ergänzt, gekürzt oder als eigener Upstreambefund erfunden.
Eine neue Registry-Archivextraktion wird nicht behauptet.

## Artefakt- und Materialprüfung

Der erhaltene native Testversionsbinärstand gehört laut damaligem Buildbeleg zu
`634d6c98ca7a320e48e8f0f89e0b636432f4f438`. Das vorhandene Binary unter
`artifacts/testversion-release-native-v2/Everlast Dictation Testversion/Everlast Dictation.app/Contents/MacOS/everlast-dictation`
wurde erneut gehasht: 15892576 Byte,
`45f4e463076c88e36adcc7ae95f810329686b32a69be374e0440d0ed7c0fd4ca`.
Historische Build-/ZIP-Belege sind im Addendum selbst gehasht gebunden. Das ist
keine neue Extraktion oder Vollständigkeitsprüfung der eingebetteten Assets.

Scoped Checks ohne Build: alle 317 Komponenten referenzieren vorhandene Texte
mit passenden Bytezahlen und SHA-256-Werten (222 verschiedene referenzierte
Textdateien); der neue Text ist bytegleich zum vollständigen Vite-Original;
die bisherigen 315 Komponentenobjekte und deren Originaltextdateien wurden
nicht geändert. Das neue `components.json` hat SHA-256
`7b8f76d1f70b2fc2331998f9b5d6b92430de0ce739d5a607c8100a0904e18a0a`.

Die reale finale arm64-Link-/Feature-/Bundleclosure bleibt ungeprüft, ebenso
gegebenenfalls zusätzliche Pflichten tatsächlich enthaltener Codeanteile.
Die fehlende originale realfft-Copyrightnotice bleibt als Quellenlücke sichtbar;
der separat bezeichnete MIT-Ergänzungstext ersetzt keinen erfundenen Inhaber.
Modellgewichte (OpenAI/ggml und Silero) bleiben getrennte Downloadquellen,
keine hiermit behaupteten gebündelten Gewichte. Kein `qualified`-, Legal- oder
Release-Freigabestatus wird aus diesem Material abgeleitet.

Der tatsächliche damalige Releasebuild verwendete den separaten Targetbaum
`artifacts/testversion-build-v1/target/`, wie im zugehörigen
`native-ui-release-paint.log` dokumentiert; kein veraltetes Workspace-Target
wurde als Buildautorität verwendet. Die hier erneut gehashte Autorität ist
das ausdrücklich konservierte Bundlebinary, nicht ein neu gebautes Target.
