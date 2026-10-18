import copy
import unittest
from scripts.screening_quality.assembly import assemble


class AssemblyTests(unittest.TestCase):
    def setUp(self):
        self.manifest = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": [
            dict(id="case-%03d" % i, language="de", split="acceptance", **{"class": "clean"}, reference="synthetic")
            for i in range(100)]}
        self.completion = dict(kind="pretranscription-continuation-execution", continuationId="new-epoch",
                               oldFreezeSha256="a" * 64, completed=True, error=None, blocks=[])
        self.authorities = []
        self.files = {}
        variants = ("PretransReference", "PretransCandidate", "PretransCandidate", "PretransReference")
        for b, variant in enumerate(variants, 1):
            block = dict(name="B%d" % b, variant=variant, cases=[])
            self.completion["blocks"].append(block)
            for i, case in enumerate(self.manifest["cases"]):
                n = (b - 1) * 100 + i
                origin = dict(epoch="a" * 64 if n < 185 else "new-epoch",
                              sourceRevision="old" if n < 185 else "new", completionPath="completion-%d" % n)
                bindings = [dict(path="%d-%s" % (n, role), bytes=1, sha256="b" * 64, mode="0o600")
                            for role in ("result", "metadata", "log")]
                rows = [dict(id=case["id"], runId=run, warmup=j == 0, status="ok", validated=True)
                        for j, run in enumerate(("warmup", "measured-001"))]
                source = dict(id=case["id"], status="ok", error=None, rows=rows, rawFiles=bindings, actualProbePid=n + 1)
                entry = dict(copy.deepcopy(source), adopted=n < 185, origin=origin)
                block["cases"].append(entry)
                authority = dict(block=block["name"], variant=variant, id=case["id"], adopted=n < 185,
                                 origin=copy.deepcopy(origin), rawFiles=copy.deepcopy(bindings),
                                 completion=dict(path=origin["completionPath"], bytes=1, sha256="c" * 64, mode="0o600"))
                self.authorities.append(authority)
                self.files[origin["completionPath"]] = source
                self.files[bindings[0]["path"]] = [dict(id=run, status="ok", warmup=j == 0, text="synthetic %d %d" % (n, j))
                                                       for j, run in enumerate(("warmup", "measured-001"))]
                self.files[bindings[1]["path"]] = {"placeholder": True}
                self.files[bindings[2]["path"]] = None

    def load(self, binding, role):
        return copy.deepcopy(self.files[binding["path"]])

    def run_assembly(self):
        return assemble(self.manifest, self.completion, self.authorities, self.load)

    def test_all_800_preserve_phase_order_text_and_origins(self):
        reports, provenance = self.run_assembly()
        self.assertEqual(len(reports), 8)
        self.assertEqual(sum(len(rows) for rows in reports.values()), 800)
        self.assertEqual(reports[("B2", "measured")][85], {"id": "case-085", "status": "ok", "text": "synthetic 185 1"})
        self.assertEqual(len(provenance["slots"]), 800)
        self.assertEqual(provenance["adoptedCases"], 185)
        self.assertEqual(provenance["newCases"], 215)

    def test_missing_duplicate_reordered_and_failed_slots_reject(self):
        base = copy.deepcopy(self.completion)
        for mutate in (lambda c: c["blocks"][0]["cases"].pop(),
                       lambda c: c["blocks"][0]["cases"].__setitem__(1, c["blocks"][0]["cases"][0]),
                       lambda c: c["blocks"].reverse(),
                       lambda c: c["blocks"][0]["cases"][0].update(status="unavailable")):
            self.completion = copy.deepcopy(base)
            mutate(self.completion)
            with self.assertRaises(ValueError):
                self.run_assembly()

    def test_stale_origin_or_unvalidated_row_reject(self):
        for change in ("origin", "validated", "raw"):
            with self.subTest(change=change):
                original = copy.deepcopy(self.completion)
                entry = self.completion["blocks"][0]["cases"][0]
                if change == "origin":
                    entry["origin"]["epoch"] = "stale"
                elif change == "validated":
                    entry["rows"][1]["validated"] = False
                else:
                    entry["rawFiles"][0]["sha256"] = "d" * 64
                with self.assertRaises(ValueError):
                    self.run_assembly()
                self.completion = original

    def test_changed_source_completion_or_raw_run_identity_reject(self):
        first = self.authorities[0]
        self.files[first["completion"]["path"]]["actualProbePid"] = 999
        with self.assertRaises(ValueError):
            self.run_assembly()
        self.setUp()
        self.files[self.authorities[0]["rawFiles"][0]["path"]][1]["id"] = "warmup"
        with self.assertRaises(ValueError):
            self.run_assembly()

    def test_no_partial_result_on_late_bad_text(self):
        self.files[self.authorities[-1]["rawFiles"][0]["path"]][1]["text"] = None
        with self.assertRaises(ValueError):
            self.run_assembly()

    def test_full_prefix_profile_cannot_accept_a_continuation_in_disguise(self):
        from scripts.screening_quality.profiles import FULL_PREFIX
        with self.assertRaises(ValueError):
            assemble(self.manifest,self.completion,self.authorities,self.load,profile=FULL_PREFIX)

    def test_full_prefix_has_800_fresh_origins_without_adoption_or_text_normalization(self):
        from scripts.screening_quality.profiles import FULL_PREFIX
        self.completion.update(kind="full-prefix-screening-execution-v1",rowCount=800,powerAssertionEnded=True,
            screeningId="screen",sourceRevision="f"*40,freezeSha256="e"*64,qualified=False,L=None)
        for index,authority in enumerate(self.authorities):
            block=self.completion["blocks"][index//100]
            block["variant"]=FULL_PREFIX.variants[index//100]
            authority["variant"]=block["variant"]
            authority.pop("adopted")
            authority["origin"]=dict(freezeSha256="e"*64,screeningId="screen",sourceRevision="f"*40,
                                     completionPath=authority["completion"]["path"])
            entry=block["cases"][index%100]
            entry.pop("adopted")
            entry.pop("origin")
            entry.update(globalSlot=index+1,blockName=block["name"],variant=block["variant"],
                         screeningId="screen",freezeSha256="e"*64)
            self.files[authority["completion"]["path"]]=copy.deepcopy(entry)
        literal="  Ä / e\u0301\nⅥ  "
        self.files[self.authorities[0]["rawFiles"][0]["path"]][0]["text"]=literal
        results,provenance=assemble(self.manifest,self.completion,self.authorities,self.load,profile=FULL_PREFIX)
        self.assertEqual(results[("B1","warmup")][0]["text"],literal)
        self.assertEqual(sum(len(value) for value in results.values()),800)
        self.assertEqual(provenance["freshCases"],400)
        self.assertNotIn("adoptedCases",provenance)
        self.assertTrue(all("adopted" not in slot and "epoch" not in slot["origin"] for slot in provenance["slots"]))
        with self.assertRaises(ValueError):self.run_assembly()


if __name__ == "__main__":
    unittest.main()

class FileBoundaryTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        import shutil
        from pathlib import Path
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve(strict=True)
        (self.root / "scripts").mkdir()
        (self.root / "artifacts").mkdir()
        source = next(
            (parent / "scripts/run-paced-remainder.py"
             for parent in Path(__file__).resolve().parents
             if (parent / "scripts/run-paced-remainder.py").is_file()),
            None)
        if source is None:
            raise RuntimeError("Public remainder source repository not found above tests")
        shutil.copyfile(source, self.root / "scripts/run-paced-remainder.py")

    def test_actual_reader_rejects_tampered_mode_bytes_hash_and_symlink(self):
        import hashlib
        from scripts.screening_quality.assembly import reader
        path = self.root / "artifacts/raw.json"
        data = b"{}"
        path.write_bytes(data)
        path.chmod(0o600)
        binding = dict(path="artifacts/raw.json", bytes=2, mode="0o600", sha256=hashlib.sha256(data).hexdigest())
        load = reader(self.root)
        self.assertEqual(load(binding, "results"), {})
        path.write_bytes(b"[]")
        with self.assertRaises(ValueError):
            load(binding, "results")
        path.write_bytes(data)
        path.chmod(0o644)
        with self.assertRaises(ValueError):
            load(binding, "results")
        path.unlink()
        target = self.root / "artifacts/target"
        target.write_bytes(data)
        target.chmod(0o600)
        path.symlink_to(target)
        with self.assertRaises(OSError):
            load(binding, "results")

    def test_private_exclusive_publication_never_overwrites(self):
        from scripts.screening_quality.assembly import publish_inputs
        fixture = AssemblyTests()
        fixture.setUp()
        results, provenance = fixture.run_assembly()
        output = self.root / "artifacts/assembled"
        publish_inputs(self.root, output, fixture.manifest, results, provenance)
        self.assertEqual(len(list(output.iterdir())), 11)
        self.assertTrue(all(p.stat().st_mode & 0o777 == 0o600 for p in output.iterdir()))
        before = (output / "B1-warmup.results.json").read_bytes()
        with self.assertRaises(FileExistsError):
            publish_inputs(self.root, output, fixture.manifest, results, provenance)
        self.assertEqual((output / "B1-warmup.results.json").read_bytes(), before)

    def test_publication_rejects_traversal_outside_artifacts(self):
        from scripts.screening_quality.assembly import publish_inputs
        fixture=AssemblyTests()
        fixture.setUp()
        results,provenance=fixture.run_assembly()
        output=self.root/"artifacts"/".."/"outside"
        with self.assertRaises(ValueError):
            publish_inputs(self.root,output,fixture.manifest,results,provenance)
        self.assertFalse((self.root/"outside").exists())

    def test_publication_rejects_parent_symlink_escape_and_final_symlink(self):
        from scripts.screening_quality.assembly import publish_inputs
        fixture=AssemblyTests()
        fixture.setUp()
        results,provenance=fixture.run_assembly()
        outside=self.root/"outside"
        outside.mkdir()
        alias=self.root/"artifacts"/"alias"
        alias.symlink_to(outside,target_is_directory=True)
        with self.assertRaises(ValueError):
            publish_inputs(self.root,alias/"escaped",fixture.manifest,results,provenance)
        self.assertFalse((outside/"escaped").exists())
        final=self.root/"artifacts"/"final"
        final.symlink_to(outside,target_is_directory=True)
        with self.assertRaises((ValueError,FileExistsError)):
            publish_inputs(self.root,final,fixture.manifest,results,provenance)
        self.assertEqual(list(outside.iterdir()),[])

    def test_canonical_root_alias_is_valid_and_remains_exclusive(self):
        from scripts.screening_quality.assembly import publish_inputs
        fixture=AssemblyTests()
        fixture.setUp()
        results,provenance=fixture.run_assembly()
        # Explicit root alias exercises the same relationship as /var ->
        # /private/var without depending on one platform's temporary directory.
        alias=self.root/"root-alias"
        alias.symlink_to(self.root,target_is_directory=True)
        output=alias/"artifacts"/"assembled-alias"
        publish_inputs(alias,output,fixture.manifest,results,provenance)
        self.assertEqual(len(list(output.iterdir())),11)
        with self.assertRaises(FileExistsError):
            publish_inputs(alias,output,fixture.manifest,results,provenance)
