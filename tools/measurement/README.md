# Isolierter FullPrefix-Ressourcenadapter V2

Dieses Verzeichnis liefert **nur reproduzierbaren Quellcode** für den bereits privat geprüften Ressourcenadapter. `full-prefix-resource-v2.json` bindet die genaue Basis, alle sechs Vorher-/Nachher-Dateien, den Patch und die 47 gemeinsamen Qualitätsquellen. Keine Modelle, Audios, Referenz-/Hypothesentexte, privaten Teststubs oder positiven Freigabeattrappen gehören zum Paket.

Der Patch gilt ausschließlich gegen `ab7b67d4e83e3b30aa8801be2b36502d312e197c`. Er enthält die fünf geprüften Exampledateien (einschließlich der im Main bisher fehlenden `identity.rs`/`observed.rs`) und die exakt achtzeilige `cfg(feature = "measurement")`-Factory `InferenceService::with_initializer`. Er übernimmt **nicht** blind den alten Diff gegen den privaten Suffixadapter. Der gesamte übrige Service bleibt bytegleich. Keine Cargo-, Feature-, Engine- oder Produktpolicyänderung.

## Tatsächlich nachgewiesen

In zwei temporären lokalen Git-Klonen wurden nur die benötigten Basisdateien ausgecheckt; `checkout-index -u` aktualisiert dabei den Index-Statcache. Ein erster Check ohne `-u` wurde von `git apply --index` wegen des fehlenden Statcache-Abgleichs abgewiesen; es wurde kein Sourcefix dafür eingeführt.

Danach erfolgreich: echter `git apply --check --index`, `git apply --index`, genau sechs geänderte Pfade, alle sechs Dateien bytegleich dem geprüften V2-Archiv SHA256 `72645fe1407444e35a1442346cdd5721ea0b712a0a2b65c1fe53ef5e85f40132`, Überprüfung aller 47 gemeinsamen Quellen, `git apply --reverse --check --index`, tatsächliches Reverse-Apply und vollständiger Datei-/Index-Roundtrip zur Basis. Beide temporären Bäume wurden entfernt. Main-Index und Produktdateien wurden nicht verändert.

Die 47 gemeinsamen Basisdateien stimmen zusätzlich exakt mit dem historischen Qualitäts-Quellstand `301fee4cea4d9c87023bf949838d3468ecba2e95` überein. Im Ergebnis bleiben 46 ganze Dateien identisch; ausschließlich die Service-Datei enthält die bekannte monotone Mess-API-Ergänzung. Deren exaktes Entfernen ergibt wieder die **gesamte** historische Datei, nicht nur ausgewählte Funktionskörper. SHA256 und Größen stehen im JSON.

**Nicht ausgeführt:** Mac-Cargo-Build, Rust-/native Tests, Modelle, Aufnahme, tatsächliche Ressourcenmessung. Historische private Pure-/Reviewbelege sind kein Mac-Compile- oder Laufzeitbeleg für dieses vollständige Paket. Ein Apply-Erfolg beweist weder Ressourcenqualifikation noch sichere Übergabe.

## Späterer, ausdrücklich freizugebender Ablauf

Jetzt nicht während einer laufenden Messung ausführen. Erst die gültige vollständige numerische **und** unabhängige semantische Qualitätsentscheidung sowie die vor Ressourcenmessung nötigen Leistungsnachweise abwarten. Kein Startrecht folgt aus diesem README.

1. Einen **neuen isolierten** Sourcebaum anlegen. Main und dessen gebundene 47 Quellen bleiben unangetastet. Beispiel ab Repositorywurzel; `SOURCE_DIR` muss fehlen:

   ```sh
   REPO_ROOT=$(git rev-parse --show-toplevel)
   SOURCE_DIR="$REPO_ROOT/artifacts/full-prefix-resource-source-v2"
   git clone --shared --no-checkout "$REPO_ROOT" "$SOURCE_DIR"
   git -C "$SOURCE_DIR" switch --detach ab7b67d4e83e3b30aa8801be2b36502d312e197c
   git -C "$SOURCE_DIR" apply --check --index "$REPO_ROOT/tools/measurement/full-prefix-resource-v2.patch"
   git -C "$SOURCE_DIR" apply --index "$REPO_ROOT/tools/measurement/full-prefix-resource-v2.patch"
   git -C "$SOURCE_DIR" diff --cached --stat
   ```

2. Patch-SHA, alle sechs Ziel-SHAs und die vollständige 47→47-Transformation anhand des JSON unabhängig rehashen. Genau sechs geänderte Pfade; neue Dateien haben `base:null`. Bei jeder Abweichung stoppen. Keine Filter-/Whitespace-Reparatur oder weitere Sourceänderung still zulassen. Danach den **isolierten** Quellstand als eigenen Sourcecommit festhalten:

   ```sh
   git -C "$SOURCE_DIR" commit -m "Prepare isolated FullPrefix resource measurement adapter v2"
   git -C "$SOURCE_DIR" rev-parse HEAD
   ```

   Dieser neue Commit, nicht die Basis oder ein behaupteter alter Qualitätscommit, ist später `sourceRevision` des Ressourcenbinarys. Ein eventueller späterer Main-`APP_POLICY`-Wechsel macht keinen falschen 47-Bytegleichheitsclaim zulässig; historische Authority bleibt separat eingefroren.

3. Erst auf dem freigegebenen Mac einen eigenen Entwickler-Build und die beiden **modellfreien** exakten Vorlagen erzeugen. Kein Appbundle/Releaseartefakt der installierten App ersetzen. Fehlender Mac-Compile-/Testnachweis ist vor echter Ausführung nachzuholen; nötige Korrekturen benötigen ein neues geprüftes Sourcepaket.

   ```sh
   (cd "$SOURCE_DIR" && cargo build --offline --release --features measurement --example runtime_resource_probe)
   PARAMETER_DIR="$REPO_ROOT/artifacts/full-prefix-resource-printed-v2"
   (umask 077; mkdir "$PARAMETER_DIR"; set -C
     "$SOURCE_DIR/target/release/examples/runtime_resource_probe" --print-full-prefix-reference-parameters > "$PARAMETER_DIR/reference.json"
     "$SOURCE_DIR/target/release/examples/runtime_resource_probe" --print-full-prefix-candidate-parameters > "$PARAMETER_DIR/candidate.json")
   ```

   Binary/Sourcen/Buildprotokoll/Vorlagen unabhängig binden. Die tatsächliche Targetdirectory muss zum Build gehören; keine fremde bestehende Binary anhand ihres Dateinamens übernehmen.

4. Den **bestehenden DualRoot-Vertrag** aus `docs/verification/full-prefix-resource-freeze-schema.json` und `scripts/run-full-prefix-resource.py` verwenden:
   - `qualityAuthorityRoot`: realer kopierter gültiger historischer Authoritybaum mit vollständiger unveränderter Freigabe und allen 47 real rehashten Quellen. `screening_quality.decision.validate` bleibt unverändert streng.
   - `resourceSourceRoot`: separater kanonischer isolierter Sourcebaum aus Schritt2; tatsächliche Dateien und immutable Sourcecommit sowie sämtliche gedruckten compiledPins prüfen. Alle 47 Transferdateien bleiben Pflicht, auch wenn einzelne Qualitätswrapper keine operative Ressourcendependency sind.
   - Eigene gebundene `sourceTransfer`- und externe `resourceMeasurementApproval`-Belege, keine automatisch erzeugte Freigabe. Der alte Qualitätsvalidator **muss** den Ressourcenbaum mit Service-Ergänzung weiterhin zurückweisen. Kein Pin-Skip/Monkeypatch/Lockerung.
   - Prospektiv geeignete, unveränderte deutsche Fixture samt Modusbaseline festlegen. Aktuell vorgesehen: kanonischer DE5-Fall `fleurs-de-test-1736`, 80000 Frames; der gebundene statische Katalog beschreibt Cut68480/früheste ideale Publikation73280. Das ist kein tatsächlicher Early-Job-Beweis. Modusbaseline erst aus den vier bereits gebundenen Warm-/Measured-Outputs der zugehörigen Qualitätsblöcke gewinnen und deren Bytegleichheit prüfen; keine neue ASR-Referenzerzeugung. Fixture-/Baseline-/Vorlagenbindung vor Ressourcenfreeze.
   - Der Launcher-Default ist Preflight; er darf nur bei echten vollständigen Autoritäten erfolgreich sein. Erst danach unabhängige Freigabe eines frischen tatsächlichen Ressourcenfreezes. Kein aktueller Freeze oder Ausführungsbefehl mit Modellpfaden wird hier geliefert.

Der bestehende Umfang bleibt fünf Warmups plus 100 Zyklen, gemessen34/33/33 (39 Stop-/66 Cancel-Zeilen insgesamt), 104 Folgestarts, tatsächliche scoped Calls/PCM-/Range-/Receipt-/Settlement-Belege, finaler Ownerjoin sowie101RSS/2vmmap. Ein statischer Pausencut ersetzt nicht die tatsächliche Early-Job-Coverage. Held-Decode wird freigegeben und abgewickelt, bevor der Folgestart Ownership bestätigt; dies behauptet keinen Neustart während eines noch gehaltenen nativen Calls.

Kein TC-Publisher-, vollständiger Feed-/L-, Liveheap-/Leak- oder Mikrofon-/Pasteclaim. RSS/vmmap sind Ressourcenbeobachtungen ohne erfundene Schwelle. Keine Aktivierung der Produkt-Vorabtranskription und keine neue Qualifikation allein durch dieses Mess-API-Paket.
