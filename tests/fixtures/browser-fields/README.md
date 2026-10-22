Aktueller statischer Validator: `validate.ts`; Ausführung vom Repository-Root
mit `node --import tsx --test tests/tooling/browser-fixture.test.ts`.
`PROVENANCE.json` ist der unveränderte historische Erstellungsbeleg, kein
Hashmanifest des heutigen Validators.

# Lokale Browserfeld-Fixture – Vorbereitung, keine native Abnahme

Stand: öffentliche Plan-/Produktquellen aus Commit
5bbe330f7b32e043113f32f9ec2de342a7a147fb. Eigenständige, netzwerkfreie Testseite;
keine App-/IPC-/ASR-Integration. Remote erstellt, nicht im Mac-Worktree.
Ziel: P02K/P07 A15b/c/d/A21 und der getrennte A21a-Transportnachweis.
Keine neuen Produktgates. Die Seite selbst sendet keine Eingaben an andere Apps.

## Was vorliegt, was noch nicht

index.html enthält18 bezeichnete Feld-/Kontextfälle und vier explizit geplante
Aktionen mit nomineller setTimeout-Verzögerung von5000ms. Browser können sie
später ausführen; vor Stop immer die Meldung „ausgeführt“ abwarten. Eine einzige Aktion darf vorgemerkt sein; Reset widerruft sie und
setzt nur die dynamischen Fälle zurück. Neuladen setzt alle Felder zurück.
Keine Eingabe-, Passwort-, Clipboard-, Konsolen- oder Netzwerkprotokollierung.
Inline-JS reagiert nur auf die eigenen Aktionsbuttons; es liest keine Feldwerte.
Systemschrift, native HTML-Controls, sichtbare Fokusmarkierung, Hell/Dunkel über
Systemfarben. Die Seite enthält keine eigene Diktierlogik oder Rechteabfrage.

fixture.test.cjs führt den tatsächlichen Inline-JS-Text mit kleinen DOM-/Timer-
Doubles aus. Das beweist Aktionsreihenfolge/Identität und verhindert versehentliches
Wertlesen an dieser Grenze, keine native Browser- oder AX-Abbildung. validate.ts
prüft den statischen Aufbau/Referenzen/Netzwerkgrenzen. Es gab KEINEN echten
Browserstart, Screenshot, AX-/TCC-/Secure-Input-/Clipboard-/Einfügetest.
matrix.json hat ausschließlich status:not-run und observations:null.

## Späterer begrenzter Mac-Prüflauf

Ohne konkurrierende Modell-/GUI-Messung:

1. Die unveränderte Seite lokal als file:// öffnen. Kein Server ist nötig.
   Browser/OS/App-Version, Kandidatenrevision, App-/Artefakt-SHA und CDHash,
   Rechtezustand, aktives Tastaturlayout, VoiceOverzustand, vorherige Browser-
   Accessibility-Aktivierung sowie Startargumente/Experimentzustand erfassen.
   Keine fremden AX-Flags schreiben. Die Seite benötigt kein Browserkonto.
2. Nur öffentliche Dummyinhalte verwenden. Vor jedem Fall neu laden oder den
   ausdrücklich passenden dynamischen Reset verwenden. Browser wird Ziel-App;
   die Diktier-App bleibt verborgen. Nicht aus dem Diktierfenster starten:
   Fensterstart ist absichtlich lokal und prüft keine automatische Übergabe.
3. Vor dem Start den angegebenen Startkontext tatsächlich fokussieren. Dessen
   native Rolle/Subrolle, PID, Enabled-/Editierbarkeitsevidenz und bei Bedarf
   Secure-Input-Status unabhängig beobachten. DOM-Fokus/Rolle beweist das nicht.
   Die vorhandenen target_probe/native_service_probe lesen nur und senden nicht.
4. Einen explizit autorisierten nativen Transportfixture-Pfad oder ein echtes
   Hotkey-/Mikrofondiktat verwenden und die Art festhalten. Dieses Paket enthält
   keinen Fixture-Sender und keinen neuen Produkt-IPC-Befehl. A21a erfordert
   getrennt echte Mikrofon/Inferenz-E2E UND unveränderte native Transportfixture;
   Datei oder manuelles Copy/Paste allein erfüllen diese Abnahme nicht.
5. Bei statischen Negativfällen sind Startziel/Clipboard/Events und tatsächlicher
   Zielinhalt zu vergleichen. Wenn der Browser das Feld nicht fokussieren kann,
   nicht behaupten, das deaktivierte Feld sei nativ geprüft worden. Das zuletzt
   fokussierte positive Feld könnte sonst weiter Ziel sein. Fall als nicht
   erreicht/inconclusive erfassen; D04 prüft separat enabled→disabled.
6. Nach Stop unterscheiden: tatsächlich korrekt eingefügt, sicher gehalten,
   Fehler, nicht erreicht oder unklar. Ein gesendetes Event ist kein Empfangs-
   nachweis; korrektes Zurückhalten ist keine positive App-Supportzusage.
   Keine automatische Wiederholung oder alternative Einfügemethode verwenden.
7. Nur öffentliche Fixtureinhalte dürfen in Belegen erscheinen. Kein echtes
   Passwort, kein privates Diktat, keine allgemeine Clipboard-Datenablage.
   Erwartung und tatsächliche Beobachtung getrennt in einer privaten Kopie der
   Matrix festhalten; Originalmatrix bleibt prospective.

## Reproduzierbare Sonderfälle

- X01 Feldwechsel: E01 fokussieren → Hotkeystart → E02 selbst fokussieren →
  Hotkeystop. Beide sind normale input[type=text] im selben Browser. Automatische
  Übergabe in E02 ist unzulässig; weder Rolle noch Seiten-ID ersetzen Feldidentität.
- D01 Ersetzung: Reset → D01-Button. Er fokussiert das alte Feld und plant einen Timer mit nominell5000ms Verzögerung.
  Sofort Hotkeystart. Die Meldung „ausgeführt“ abwarten; der neue Knoten ist
  frisch/leer, aber id/type/Label gleich. Er wird durch DIESE TESTSEITE fokussiert.
  Erst dann stoppen. Das neue Feld darf nicht als altes Startfeld gelten.
- D02/D04: gleicher Ablauf mit Sperren/Deaktivieren. Der alte Knoten bleibt
  erhalten, seine Editierbarkeit ändert sich. Browser können beim disabled-
  Wechsel den Fokus verlieren; die tatsächliche native Ablehnungsursache notieren.
  Es wird kein bestimmter AX-Fehlercode vorausgesetzt.
- D03 spätes Ziel: Reset muss Container ohne Textfeld ergeben. D03-Button →
  Containerfokus → sofort Hotkeystart → auf „ausgeführt“ und neues Gesprächsfeld warten
  (nominell5000ms, möglicherweise später) →
  stoppen. Neu auftauchendes Feld darf fehlendes Startziel niemals ersetzen.
  Die Fokusaktion der Testseite ist explizit und wird nicht der App zugeschrieben.
- L01: „A“ im datalist-Feld eingeben, Pfeil abwärts. Popup/AX-Fokus erfassen.
  Browserabhängig; kein garantierter nativer Autofill-Reproducer. Falls tatsächlich
  ein Liste-/Container-Kontext statt Startfeld erfasst wird, muss er gehalten
  bleiben. Bleibt nativ ein normales gültiges Feld fokussiert, wurde der negative
  Autofillfall damit NICHT erreicht.
- L02: eigener Button fokussiert ausschließlich die synthetische ARIA-option.
  Das prüft einen expliziten Nicht-Textfeld-Kontext, nicht native Autofill-Interna.
- S01: ausschließlich Dummytext in type=password. Erkanntes Passwortziel darf
  keinen automatischen Versand erhalten. Secure Input separat nativ messen;
  Passwortfeld bedeutet nicht automatisch aktive globale sichere Eingabe.
  Für Beginn sicherer Eingabe WÄHREND Aufnahme separat E01 starten, dann selbst
  S01 fokussieren; Stop/Escape/unerwünschtes druckbares Hotkeyzeichen beobachten.
  Nur wenn Secure Input tatsächlich aktiv beobachtet ist, diese Variante als
  Secure-Input-Test verbuchen. Fehlenden Nachweis nicht als Pass eintragen.

## Rollen-/Empfängermatrix und Abgrenzung

matrix.json enthält präzise Aktionen/Erwartungen für die Feldfälle und zwei
zusätzliche Workflows X01/A21a-Transport. Browserläufe sind getrennt für Safari,
Chrome/Experiment an/aus und Firefox zu erfassen, jeweils Erststart, Navigation/
Reload und VoiceOver an/aus soweit A21 verlangt. Chrome-Vorbedingungen und
tatsächlich wirksame Flags dokumentieren; keine Wirkung nur aus CLI behaupten.

TextEdit, Google Docs in Chrome, Slack, Discord, Notion, VS Code und Terminal
bleiben eigene tatsächliche App-/Feldfälle. Diese lokale Seite bildet ihre
Implementierung nicht nach und ersetzt ihre A21-Abnahme nicht. Ebenso bleiben
GNU Emacs/Zed/Terminal/VS Code-Hotkeyunterdrückung, Layouts/Modifier, Clipboard-
Restore/Providergrenzen und P02-Mikrofon-E2E gesondert. Die Seite wechselt weder
Layout noch Rechte, VO, Secure Input oder Systempräferenzen.

transport-fixture.txt ist öffentliche, vorab feste UTF-8-Fracht mit Umlauten,
Ziffern, Zeilenumbruch und terminalem LF. Der Transporttest muss diese Bytes
einschließlich Zeilenumbrüchen erhalten. Keine ASR-Textgenauigkeit daraus ableiten.
Passende Inhalte selbst vorzulesen ersetzt den gesonderten Fixture-Transport nicht.

## Reine Remote-Prüfbefehle

node --test fixture.test.cjs
node --import tsx --test tests/tooling/browser-fixture.test.ts

Keine Abhängigkeiten/Downloads. Native Browserausführung und visuelle Prüfung
sind ausdrücklich noch nicht erfolgt. Root prüft dieses Paket unabhängig;
es ist kein Review der Produktimplementierung oder Abschluss von A15/A21.
