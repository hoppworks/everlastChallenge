"""Synthetic payloads with actual pinned public v2 freeze/archive field shapes."""
import copy
import importlib.util
import json
import unittest
from pathlib import Path
from scripts.screening_quality.continuation_adapter import resolve, recorded_root
from scripts.screening_quality.assembly import assemble

def fixture_root():
    markers = (
        "scripts/run-pretranscription-continuation.py",
        "scripts/run-paced-remainder.py",
        "tests/fixtures/corpus/p06-pretranscription-screening-run-v2.json",
    )
    for parent in Path(__file__).resolve().parents:
        if all((parent / name).is_file() for name in markers):
            return parent
    raise RuntimeError("Public continuation source/fixture repository not found above tests")


BASE = fixture_root()
SPEC = importlib.util.spec_from_file_location("continuation_protocol", BASE / "scripts/run-pretranscription-continuation.py")
PROTOCOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROTOCOL)


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.old = json.loads((BASE / PROTOCOL.OLD_FREEZE_PATH).read_text())
        self.archive = json.loads((BASE / PROTOCOL.OLD_ARCHIVE_PATH).read_text())
        self.cid = "12345678-1234-4123-8123-123456789012"
        self.ledger = copy.deepcopy(PROTOCOL.build_ledger(self.old, self.archive))
        self.ledger["continuationId"] = self.cid
        self.epoch = copy.deepcopy(self.old)
        self.epoch["sourceRevision"] = "e" * 40
        self.epoch["outputDirectory"] = "artifacts/synthetic-continuation"
        self.root = recorded_root(self.old)
        for block in self.epoch["blocks"]:
            for c in block["cases"]:
                for k in ("output", "metadata", "log", "completion"):
                    c[k] = str(Path(self.epoch["outputDirectory"]) / Path(c[k]).name)
                c["command"][8] = str(self.root / c["output"])
        self.wrapper = dict(schemaVersion=1, kind="prospective-pretranscription-continuation-v1",
                            continuationId=self.cid, sourceRevision=self.epoch["sourceRevision"],
                            originalFreeze=dict(sha256=PROTOCOL.OLD_FREEZE_SHA),
                            originalArchive=dict(sha256=PROTOCOL.OLD_ARCHIVE_SHA),
                            originalCompletion=self.archive["bindings"]["completion"],
                            epochFreeze=dict(sha256="2"*64), adoptionLedger=dict(sha256="3"*64),
                            adoptedPrefixCases=185, newCases=215, rerunFromSlot=186,
                            v3AdoptionAllowed=False, qualified=False, L=None)
        self.wrapper_binding = dict(path="synthetic-wrapper.json", sha256="1"*64, bytes=1, mode="0o600")
        self.original_global = dict(completed=False, blocks=[])
        self.done = dict(schemaVersion=1, kind="pretranscription-continuation-execution",
                         continuationId=self.cid, oldFreezeSha256=PROTOCOL.OLD_FREEZE_SHA,
                         freezeSha256="2"*64, wrapperSha256="1"*64, adoptionLedgerSha256="3"*64,
                         completed=True, error=None, rowCount=800, plannedRows=800,
                         adoptedCases=185, adoptedRows=370, newCasesRequested=215,
                         newLaunchAttempts=215, newRowsRequested=430, powerAssertionEnded=True,
                         qualified=False, L=None, blocks=[])
        self.files = {}
        self.bindings = {}
        self.manifest = json.loads((BASE / self.old["manifest"]["path"]).read_text())
        for variant, binding in self.epoch["parameters"].items():
            self.files[binding["path"]] = json.loads((BASE / binding["path"]).read_text())
        ordinal = 0
        for oldblock, block in zip(self.old["blocks"], self.epoch["blocks"]):
            old_projection = dict(name=block["name"], variant=block["variant"], cases=[])
            projection = dict(name=block["name"], variant=block["variant"], cases=[])
            self.original_global["blocks"].append(old_projection)
            self.done["blocks"].append(projection)
            for ci, c in enumerate(block["cases"]):
                ordinal += 1
                if ordinal <= 185:
                    public = self.archive["cases"][ordinal - 1]
                    ref = self.ledger["entries"][ordinal - 1]
                    source = {k: copy.deepcopy(public[k]) for k in
                              ("id", "status", "error", "rows", "launch", "actualProbePid", "beforePower", "afterPower")}
                    source["rawFiles"] = [dict(r, path=str(self.root / r["path"])) for r in ref["rawFiles"]]
                    self.files[ref["completion"]["path"]] = copy.deepcopy(source)
                    old_projection["cases"].append(copy.deepcopy(source))
                    entry = dict(copy.deepcopy(source), adopted=True, origin=dict(epoch=PROTOCOL.OLD_FREEZE_SHA,
                                 sourceRevision=self.old["sourceRevision"], completionPath=ref["completion"]["path"]))
                else:
                    old_projection["cases"].append(dict(id=c["id"], status="unavailable"))
                    raw = [dict(path=str(self.root/c[k]), mode="0o600", bytes=2, sha256="f"*64)
                           for k in ("output", "metadata", "log")]
                    source = dict(id=c["id"], status="ok", error=None,
                                  rows=[dict(id=c["id"], runId=r, warmup=j==0, status="ok", validated=True)
                                        for j, r in enumerate(("warmup", "measured-001"))],
                                  launch=dict(wrapperPid=ordinal+10000, startedUtc="synthetic", exitCode=0,
                                              terminationConfirmed=True, launchError=None),
                                  actualProbePid=ordinal+20000, rawFiles=raw,
                                  beforePower=dict(source="AC Power", error=None),
                                  afterPower=dict(source="AC Power", error=None))
                    entry = dict(source, adopted=False, origin=dict(epoch=self.cid,
                                 sourceRevision=self.epoch["sourceRevision"], completionPath=c["completion"]))
                    binding = dict(path=c["completion"], mode="0o600", bytes=2, sha256="d"*64)
                    self.bindings[c["completion"]] = binding
                    self.files[c["completion"]] = copy.deepcopy(entry)
                projection["cases"].append(entry)
                self.files[entry["rawFiles"][0]["path"]] = [
                    dict(id=r, warmup=j==0, status="ok", text="synthetic only")
                    for j, r in enumerate(("warmup", "measured-001"))]
                self.files[entry["rawFiles"][1]["path"]] = None
                self.files[entry["rawFiles"][2]["path"]] = None

    def load(self, binding, role):
        return copy.deepcopy(self.files[binding["path"]])

    def run_resolve(self):
        return resolve(self.wrapper, self.old, self.archive, self.original_global, self.ledger,
                       self.epoch, self.done, self.load, lambda p: self.bindings[p], PROTOCOL, self.wrapper_binding)

    def test_real_public_shape_resolves185old215new_and_absolute_paths(self):
        authorities = self.run_resolve()
        self.assertEqual(len(authorities), 400)
        self.assertEqual(sum(a["adopted"] for a in authorities), 185)
        self.assertEqual(authorities[184]["origin"]["epoch"], PROTOCOL.OLD_FREEZE_SHA)
        self.assertEqual(authorities[185]["origin"]["epoch"], self.cid)
        self.assertEqual(authorities[185]["id"], "constructed-de-masked-300s")
        for authority in authorities:
            self.files[authority["rawFiles"][1]["path"]] = copy.deepcopy(authority["metadata"])
        results, sidecar = assemble(self.manifest, self.done, authorities, self.load)
        self.assertEqual(sum(len(v) for v in results.values()), 800)
        self.assertEqual(len(sidecar["slots"]), 800)
        self.assertEqual(authorities[185]["metadata"]["model"]["id"], "large-v3-turbo-q5_0")
        self.files[authorities[-1]["rawFiles"][1]["path"]]["sourceRevision"] = "stale"
        with self.assertRaises(ValueError):
            assemble(self.manifest, self.done, authorities, self.load)

    def test_global_self_reported_origin_cannot_replace_authority(self):
        self.done["blocks"][1]["cases"][85]["origin"]["sourceRevision"] = "wrong"
        with self.assertRaises(ValueError):
            self.run_resolve()

    def test_ledger_mutation_and_replaced_original_global_reject(self):
        self.ledger["entries"][0]["completion"]["sha256"] = "0"*64
        with self.assertRaises(ValueError):
            self.run_resolve()
        self.setUp()
        self.original_global["blocks"][0]["cases"][0]["actualProbePid"] = 1
        with self.assertRaises(ValueError):
            self.run_resolve()

    def test_shared_asset_and_new_source_mismatch_reject(self):
        self.epoch["models"][0]["sha256"] = "0"*64
        with self.assertRaises(ValueError):
            self.run_resolve()
        self.setUp()
        self.epoch["sourceRevision"] = "wrong"
        with self.assertRaises(ValueError):
            self.run_resolve()

    def test_missing_whole_case_unavailable_and_new_sidecar_mismatch_reject(self):
        self.done["completed"] = False
        with self.assertRaises(ValueError):
            self.run_resolve()
        self.setUp()
        path = self.epoch["blocks"][1]["cases"][85]["completion"]
        self.files[path]["rows"][1]["validated"] = False
        with self.assertRaises(ValueError):
            self.run_resolve()

    def test_all_three_global_epoch_bindings_are_required_and_exact(self):
        pristine = copy.deepcopy(self.done)
        for key in ("freezeSha256", "wrapperSha256", "adoptionLedgerSha256"):
            for missing in (True, False):
                with self.subTest(key=key, missing=missing):
                    self.done = copy.deepcopy(pristine)
                    if missing:
                        self.done.pop(key)
                    else:
                        self.done[key] = "0"*64
                    with self.assertRaises(ValueError):
                        self.run_resolve()
