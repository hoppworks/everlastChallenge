import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("screening_comparison", ROOT/"scripts/compare-screening-quality.py")
tool = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tool)


def selection():
    return json.loads((ROOT/"tests/fixtures/corpus/p06-context-quality-selection-v1.json").read_bytes())


def report(cases, changes=None):
    changes = changes or {}
    rows = []
    for c in cases:
        authority=json.loads(tool.DENOMINATORS.read_bytes())
        expected={r["id"]:r["refWords"] for r in authority["cases"]}
        n = expected[c["id"]]
        edits = changes.get(c["id"], 0)
        rows.append(dict(id=c["id"], split=c["split"], language=c["language"], **{"class":c["class"]},
                         status="ok", edits=edits if n else None, refWords=n,
                         hypWords=n, wer=edits/n if n else None, unwantedText=False))
    groups = {}
    for r in rows:
        key = (r["split"],r["language"],r["class"])
        g = groups.setdefault(key,dict(split=key[0],language=key[1],**{"class":key[2]},
            caseCount=0,scoredCount=0,failedCount=0,missingCount=0,silenceCount=0,
            silenceFailedCount=0,unwantedTextCount=0,expectedRefWords=0,refWords=0,edits=0))
        g["caseCount"] += 1
        g["expectedRefWords"] += r["refWords"]
        g["silenceCount"] += r["refWords"] == 0
        if r["refWords"]:
            g["scoredCount"] += 1
            g["refWords"] += r["refWords"]
            g["edits"] += r["edits"]
    for g in groups.values():
        g.update(complete=True,observedWer=g["edits"]/g["refWords"] if g["refWords"] else None,
                 wer=g["edits"]/g["refWords"] if g["refWords"] else None)
    return dict(schemaVersion=1,unicodeVersion="13.0.0",cases=rows,groups=list(groups.values()))


def reports(cases):
    return {(phase,block):report(cases) for phase in ("warmup","measured") for block in (1,2,3,4)}


class ComparisonTests(unittest.TestCase):
    def test_missing_case_is_not_a_successful_subset(self):
        selected=selection()
        values=reports(selected["cases"])
        values[("measured",2)]["cases"].pop()
        with self.assertRaises(ValueError):
            tool.compare(selected,values)

    def test_exact_half_percentage_point_and_direction_for_both_pairs(self):
        selected=selection()
        cases=selected["cases"]
        chosen=next(c["id"] for c in cases if c["sourceManifest"] == tool.ORIGINAL and c["language"] == "de" and c["class"] == "challenging")
        values=reports(cases)
        self.assertTrue(tool.compare_counts(0,1,200,False)["passed"])
        self.assertFalse(tool.compare_counts(0,2,200,False)["passed"])
        values[("measured",2)]=report(cases,{chosen:2})  #original559words:two allowed,three exceed0.5pp
        result=tool.compare(selected,values)
        self.assertTrue(result["numericalScreenPassed"])
        values[("measured",2)]=report(cases,{chosen:3})
        self.assertFalse(tool.compare(selected,values)["numericalScreenPassed"])
        #Pair4/3: worsening block4 is an improved candidate, not a reversal.
        values=reports(cases)
        values[("measured",4)]=report(cases,{chosen:3})
        self.assertTrue(tool.compare(selected,values)["numericalScreenPassed"])
        values=reports(cases)
        values[("measured",3)]=report(cases,{chosen:3})
        self.assertFalse(tool.compare(selected,values)["numericalScreenPassed"])

    def test_warmup_and_supplements_cannot_be_used_to_dilute_original_groups(self):
        selected=selection()
        cases=selected["cases"]
        chosen=next(c["id"] for c in cases if c["sourceManifest"] == tool.ORIGINAL and c["language"] == "de" and c["class"] == "challenging")
        extras=[c["id"] for c in cases if c["sourceManifest"] != tool.ORIGINAL and c["expectedOutcome"] == "Local"]
        values=reports(cases)
        values[("warmup",2)]=report(cases,{chosen:3})
        for role in values:
            if role[1] in (1,4):
                values[role]=report(cases,{ident:1 for ident in extras})
        self.assertFalse(tool.compare(selected,values)["numericalScreenPassed"])

    def test_wrong_counts_sums_ids_and_classes_are_rejected(self):
        selected=selection()
        for mutation in (
            lambda r:r["cases"][0].update(refWords=201),
            lambda r:r["groups"][0].update(edits=1),
            lambda r:r["cases"][0].update(id=r["cases"][1]["id"]),
            lambda r:r["cases"][0].update(language="en"),
            lambda r:r["cases"][0].update(id="unknown-case"),
            lambda r:r["groups"][0].update(caseCount=True),
            lambda r:r["groups"][0].update(complete=False),
        ):
            values=reports(selected["cases"])
            mutation(values[("measured",2)])
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                tool.compare(selected,values)

    def test_same_denominator_manipulation_in_all_eight_reports_is_not_trusted(self):
        selected=selection()
        values=reports(selected["cases"])
        for r in values.values():
            row=r["cases"][0]
            row.update(refWords=row["refWords"]+1,hypWords=row["hypWords"]+1)
            g=next(g for g in r["groups"] if all(g[k]==row[k] for k in ("split","language","class")))
            g["refWords"]+=1
            g["expectedRefWords"]+=1
        with self.assertRaises(ValueError):
            tool.compare(selected,values)


class CliTests(unittest.TestCase):
    def test_cli_publishes_private_no_clobber_report_with_all_source_hashes(self):
        import tempfile,subprocess,sys
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            source=selection()
            paths=[]
            for block in (1,2,3,4):
                for phase in ("warmup","measured"):
                    p=root/("%d-%s.json"%(block,phase))
                    p.write_text(json.dumps(report(source["cases"])))
                    paths.append(p)
            output=root/"new.json"
            command=[sys.executable,str(ROOT/"scripts/compare-screening-quality.py"),
                     str(ROOT/"tests/fixtures/corpus/p06-context-quality-selection-v1.json"),
                     *map(str,paths),"--output",str(output)]
            result=subprocess.run(command,capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertTrue(output.exists())
            data=output.read_bytes()
            parsed=json.loads(data)
            self.assertEqual(len(parsed["sourceReports"]),8)
            self.assertFalse(parsed["qualified"])
            self.assertIsNone(parsed["L"])
            self.assertEqual(output.stat().st_mode & 0o777,0o600)
            result=subprocess.run(command,capture_output=True,text=True,timeout=10)
            self.assertNotEqual(result.returncode,0)
            self.assertEqual(output.read_bytes(),data)


class CompletenessTests(unittest.TestCase):
    def test_failed_warmup_is_preserved_and_blocks_numeric_pass(self):
        selected=selection()
        values=reports(selected["cases"])
        failed=values[("warmup",1)]
        failed["cases"][0].update(status="failed",edits=None,hypWords=None,wer=None)
        group=next(g for g in failed["groups"] if (g["split"],g["language"],g["class"])==("acceptance","de","clean"))
        #Canonical full100 acceptance-DE-clean includes the32-word supplement:44/947; failed18 ->43/929.
        group.update(scoredCount=43,failedCount=1,refWords=929,complete=False,observedWer=0.0,wer=None)
        result=tool.compare(selected,values)
        self.assertFalse(result["numericalScreenPassed"])
        self.assertEqual(result["phases"][0]["pairs"][0]["failedCases"][0]["status"],"failed")
        group["complete"]=True
        with self.assertRaises(ValueError):
            tool.compare(selected,values)

    def test_silence_unwanted_raw_punctuation_blocks_numeric_pass(self):
        selected=selection()
        values=reports(selected["cases"])
        candidate=values[("measured",2)]
        row=next(r for r in candidate["cases"] if r["refWords"]==0)
        row["unwantedText"]=True  #canonical punctuation-only output still has hypWords0
        g=next(g for g in candidate["groups"] if all(g[k]==row[k] for k in ("split","language","class")))
        g["unwantedTextCount"]=1
        result=tool.compare(selected,values)
        self.assertFalse(result["numericalScreenPassed"])
        self.assertEqual(len(result["phases"][1]["pairs"][0]["unwantedSilence"]),1)

    def test_independent_fixture_and_original_numerators_are_bound(self):
        selected=selection()
        result=tool.compare(selected,reports(selected["cases"]))
        groups=result["phases"][1]["pairs"][0]["original90"]
        indexed={(g["language"],g["class"]):(g["caseCount"],g["referenceTokens"]) for g in groups}
        self.assertEqual(indexed,{("de","clean"):(43,915),("de","challenging"):(8,559),
                                  ("en","clean"):(32,734),("en","challenging"):(7,99)})
        changed=json.loads(tool.DENOMINATORS.read_bytes())
        changed["cases"][0]["refWords"]+=1
        with self.assertRaises(ValueError):
            tool.compare(selected,reports(selected["cases"]),changed)
        values=reports(selected["cases"])
        values[("measured",2)]["cases"][0]["refWords"]=True
        with self.assertRaises(ValueError):
            tool.compare(selected,values)

    def test_clean_absolute_limit_uses_integers_not_rounded_display(self):
        self.assertTrue(tool.compare_counts(19,20,200,True)["passed"])
        result=tool.compare_counts(21,21,200,True)
        self.assertTrue(result["deltaWithinHalfPercentagePoint"])
        self.assertFalse(result["candidateWithinAbsoluteCleanLimit"])
        with self.assertRaises(ValueError):
            tool.compare_counts(False,0,200,True)


class InvalidCliTests(unittest.TestCase):
    def test_duplicate_nan_and_oversized_json_never_publish_a_green_report(self):
        import tempfile,subprocess,sys
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            selected=selection()
            paths=[]
            for i in range(8):
                path=root/("%d.json"%i)
                path.write_text(json.dumps(report(selected["cases"])))
                paths.append(path)
            command=[sys.executable,str(ROOT/"scripts/compare-screening-quality.py"),
                     str(ROOT/"tests/fixtures/corpus/p06-context-quality-selection-v1.json"),
                     *map(str,paths),"--output",str(root/"new.json")]
            for payload in ('{"x":1,"x":2}','{"x":NaN}'):
                paths[0].write_text(payload)
                result=subprocess.run(command,capture_output=True,text=True,timeout=10)
                self.assertEqual(result.returncode,2)
                self.assertFalse((root/"new.json").exists())
                self.assertNotIn(payload,result.stdout+result.stderr)
            with paths[0].open("wb") as stream:
                stream.truncate(16*1024*1024+1)
            result=subprocess.run(command,capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,2)
            self.assertFalse((root/"new.json").exists())

    def test_numeric_failure_is_exported_not_filtered_and_has_nonzero_exit(self):
        import tempfile,subprocess,sys
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            selected=selection()
            chosen=next(c["id"] for c in selected["cases"] if c["sourceManifest"]==tool.ORIGINAL and c["language"]=="de" and c["class"]=="challenging" and c["expectedOutcome"]=="Local")
            paths=[]
            for block in (1,2,3,4):
                for phase in ("warmup","measured"):
                    p=root/("%d-%s.json"%(block,phase))
                    p.write_text(json.dumps(report(selected["cases"],{chosen:3} if (block,phase)==(2,"measured") else None)))
                    paths.append(p)
            output=root/"failed-numeric.json"
            result=subprocess.run([sys.executable,str(ROOT/"scripts/compare-screening-quality.py"),
                str(ROOT/"tests/fixtures/corpus/p06-context-quality-selection-v1.json"),
                *map(str,paths),"--output",str(output)],capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,1,result.stderr)
            parsed=json.loads(output.read_bytes())
            self.assertFalse(parsed["numericalScreenPassed"])
            self.assertEqual(len(parsed["phases"]),2)
            self.assertEqual(len(parsed["phases"][1]["pairs"]),2)
            self.assertEqual(len(parsed["phases"][1]["pairs"][0]["supplement10"]),10)
            self.assertFalse(parsed["qualified"])


class EmptyConsistencyTests(unittest.TestCase):
    def test_silence_tokens_cannot_claim_no_unwanted_raw_output(self):
        selected=selection()
        values=reports(selected["cases"])
        row=next(r for r in values[("measured",2)]["cases"] if r["id"]=="synthetic-de-silence-5s")
        row.update(hypWords=1,unwantedText=False)
        with self.assertRaises(ValueError):
            tool.compare(selected,values)

    def test_missing_requires_absent_text_counts_but_failed_partial_text_is_retained(self):
        selected=selection()
        base=report(selected["cases"])
        quiet=next(r for r in base["cases"] if r["id"]=="synthetic-de-silence-5s")
        for status,hyp,unwanted,valid in (
            ("missing",None,None,True),
            ("missing",0,None,False),
            ("missing",None,False,False),
            ("failed",None,None,True),
            ("failed",0,False,True),
            ("failed",0,True,True),
            ("failed",1,True,True),
            ("failed",1,False,False),
            ("failed",None,False,False),
        ):
            report_value=copy.deepcopy(base)
            row=next(r for r in report_value["cases"] if r["id"]==quiet["id"])
            row.update(status=status,hypWords=hyp,unwantedText=unwanted)
            group=next(g for g in report_value["groups"] if all(g[k]==row[k] for k in ("split","language","class")))
            group.update(failedCount=1,missingCount=int(status=="missing"),silenceFailedCount=1,
                         unwantedTextCount=int(unwanted is True),complete=False,wer=None)
            with self.subTest(status=status,hyp=hyp,unwanted=unwanted):
                if valid:
                    indexed=tool.validate_report(selected,report_value)
                    self.assertEqual(indexed[quiet["id"]]["hypWords"],hyp)
                else:
                    with self.assertRaises(ValueError):
                        tool.validate_report(selected,report_value)
