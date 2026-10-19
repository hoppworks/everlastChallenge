import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from screening_quality import historical_reference as history


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                     separators=(",", ":")).encode()).hexdigest()


class HistoricalReferenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.manifest_bytes = (ROOT / "tests/fixtures/corpus/p06-context-quality-manifest-v1.json").read_bytes()
        self.manifest = json.loads(self.manifest_bytes)
        self.ids = [case["id"] for case in self.manifest["cases"]]
        self.roles = ("FullPrefixReference", "FullPrefixCandidate", "FullPrefixCandidate", "FullPrefixReference")
        self.source = history.SOURCE_REVISION
        self.freeze = dict(schemaVersion=1,kind="prospective-full-prefix-screening-v1",sourceRevision=self.source,
            screeningId="synthetic-bound-history",qualified=False,L=None,blocks=[],parameters={},sources=[],
            models=[dict(path="models/reference",bytes=574041195,sha256="394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",mode="0o644"),
                    dict(path="models/vad",bytes=885098,sha256="29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf",mode="0o600")])
        self.runtime = dict(syntheticFixedRuntime=True)
        self.pins = [dict(filename="old/%02d.rs" % i,sha256=hashlib.sha256(str(i).encode()).hexdigest()) for i in range(47)]
        self.freeze["sources"] = [dict(path=p["filename"],sha256=p["sha256"],bytes=1,mode="0o644") for p in self.pins]
        self.params = {role:dict(runtime=self.runtime,fullPrefix=dict(candidate=(role=="FullPrefixCandidate")),sourceHashes=self.pins) for role in set(self.roles)}
        self.completion = dict(schemaVersion=1,kind="full-prefix-screening-execution-v1",completed=True,error=None,
            sourceRevision=self.source,screeningId=self.freeze["screeningId"],rowCount=800,powerAssertionEnded=True,qualified=False,L=None,blocks=[])
        self.arrays = {}
        for i,role in enumerate(self.roles):
            name="B%d"%(i+1)
            self.freeze["blocks"].append(dict(name=name,variant=role,cases=[dict(id=x,completion="old/%s-%s.json"%(name,x)) for x in self.ids]))
            self.completion["blocks"].append(dict(name=name,variant=role,cases=[dict(id=x,status="ok",error=None,
                rows=[dict(status="ok",validated=True,warmup=w) for w in (True,False)]) for x in self.ids]))
            for phase in ("warmup","measured"):
                self.arrays[name,phase]=[dict(id=x,status="ok",text="synthetic immutable %d"%j) for j,x in enumerate(self.ids)]
        self.mutate = lambda: None

    def write(self, path, data):
        p=self.root/path;p.parent.mkdir(parents=True,exist_ok=True)
        raw=data if isinstance(data,bytes) else (json.dumps(data,ensure_ascii=False,indent=2)+"\n").encode()
        p.write_bytes(raw);os.chmod(p,0o600)
        return dict(path=path,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),mode="0o600")

    def authority(self):
        manifest=self.write("history/manifest.json",self.manifest_bytes)
        self.freeze["manifest"]=manifest
        self.freeze["parameters"]={role:self.write("history/"+role+".json",p) for role,p in self.params.items()}
        freeze=self.write("history/freeze.json",self.freeze)
        self.completion["freezeSha256"]=freeze["sha256"]
        completion=self.write("history/completion.json",self.completion)
        origin=dict(kind="full-prefix-quality-assembly-v1",unicodeVersion="13.0.0",qualified=False,L=None,
                    authorityBindings=dict(freeze=freeze,completion=completion,manifest=manifest,parameters=self.freeze["parameters"]),
                    inputs=[],slots=[])
        arraybindings=[]
        for block,phase in self.arrays:
            array=self.arrays[block,phase]
            origin["inputs"].append(dict(block=block,phase=phase,rows=100,canonicalSha256=digest(array)))
            arraybindings.append(self.write("history/"+block+"-"+phase+".results.json",array))
        for b,role in enumerate(self.roles):
            for i,case in enumerate(self.manifest["cases"]):
                for phase in ("warmup","measured"):
                    row=self.arrays["B%d"%(b+1),phase][i]
                    raw=dict(row,id="warmup" if phase=="warmup" else "measured-001",warmup=phase=="warmup")
                    origin["slots"].append(dict(slot=b*100+i+1,block="B%d"%(b+1),variant=role,caseIndex=i+1,id=case["id"],phase=phase,
                        rawRunId=raw["id"],sourceRawRowCanonicalSha256=digest(raw),referenceUtf8Sha256=hashlib.sha256(case["reference"].encode()).hexdigest(),
                        origin=dict(sourceRevision=self.source,screeningId=self.freeze["screeningId"],freezeSha256=freeze["sha256"],completionPath=self.freeze["blocks"][b]["cases"][i]["completion"])))
        originbinding=self.write("history/origin.json",origin)
        inputs=self.write("history/input-bindings.json",dict(kind="complete-eight-quality-inputs",bindings=[manifest,originbinding]+arraybindings,qualified=False,L=None))
        numeric=self.write("history/numeric.json",dict(kind="independent-full-prefix-canonical-numeric-readback-v1",auditPassed=True,
            freeze=freeze,manifest=manifest,inputBindings=inputs,origin=originbinding,canonicalUnicodeVersion="13.0.0",casesEach=100,
            all800RawTextValuesCopiedExactly=True,eightCanonicalCliExit0Recorded=True,numericalScreenPassed=True))
        semantic=self.write("history/semantic.json",dict(kind="full-prefix-completed-semantic-review-v1",sourceRevision=self.source,screeningId=self.freeze["screeningId"],
            bindings=dict(freeze=freeze,globalCompletion=completion),summary=dict(caseSlots=400,rows=800,distinctGoldCases=100),
            semanticNewErrorAssessment=dict(completePairedPhaseCoverage=True,newLostNegationObserved=False,newWrongNumberObserved=False,
                newSegmentOmissionObserved=False,newSegmentDuplicationObserved=False,newAnchorFailureObserved=False)))
        report=self.write("history/report.json",dict(schemaVersion=1,kind="full-prefix-quality-screening-decision-v1",sourceRevision=self.source,
            screeningId=self.freeze["screeningId"],qualified=False,L=None,bindings=dict(screeningFreeze=freeze,screeningCompletion=completion,
                assembledInputs=inputs,assemblyOrigin=originbinding,independentNumericReview=numeric,independentSemanticReview=semantic),
            canonicalEvaluation=dict(pythonVersion="3.9.6",unicodeVersion="13.0.0",separateReports=8,casesPerReport=100),
            decision=dict(numericalScreenPassed=True,semanticScreenPassed=True,candidateRejected=False,productionFullPrefixEnabled=False)))
        return self.write("history/approval.json",dict(schemaVersion=1,kind="full-prefix-quality-decision-v1",passed=True,decision="eligible-for-latency-measurement",productionActivated=False,
            screeningFreeze=freeze,screeningCompletion=completion,qualityReport=report,semanticReview=semantic,independentNumericReview=numeric,runtime=self.runtime))

    def test_candidate_block_cannot_be_adopted_as_historical_reference(self):
        self.freeze["blocks"][0]["variant"]="FullPrefixCandidate"
        with self.assertRaisesRegex(ValueError,"role"):
            history.load(self.root,self.authority())


    def test_original_four_arrays_are_bytes_and_never_new_run_authority(self):
        binding=self.authority()
        before={p:p.read_bytes() for p in self.root.rglob("*") if p.is_file()}
        result=history.load(self.root,binding)
        self.assertEqual([(b,p) for b,p,_ in result["reference_arrays"]],
                         [("B1","warmup"),("B1","measured"),("B4","warmup"),("B4","measured")])
        for block,phase,data in result["reference_arrays"]:
            self.assertEqual(data,(self.root/("history/"+block+"-"+phase+".results.json")).read_bytes())
        self.assertEqual(result["manifest_bytes"],self.manifest_bytes)
        self.assertEqual(result["source_revision"],self.source)
        self.assertIs(result["reuse_authorized"],False)
        self.assertIs(result["requires_bound_external_reference_equivalence"],True)
        self.assertIsNone(result["new_candidate_approval"])
        self.assertIsNone(result["new_timing"])
        self.assertEqual(before,{p:p.read_bytes() for p in before})

    def test_missing_or_reordered_ids_and_wrong_source_or_model_reject(self):
        original=copy.deepcopy((self.freeze,self.completion,self.params))
        mutations=[
            lambda:self.completion["blocks"][3]["cases"].pop(),
            lambda:self.completion["blocks"][3]["cases"].reverse(),
            lambda:self.freeze.update(sourceRevision="a"*40),
            lambda:self.freeze["models"][0].update(sha256="0"*64),
            lambda:self.params["FullPrefixReference"]["fullPrefix"].update(candidate=True),
        ]
        for index,mutate in enumerate(mutations):
            self.freeze,self.completion,self.params=copy.deepcopy(original)
            mutate()
            with self.subTest(index=index),self.assertRaises(ValueError):
                history.load(self.root,self.authority())

    def test_changed_text_or_hash_is_not_silently_adopted(self):
        binding=self.authority()
        path=self.root/"history/B4-measured.results.json"
        raw=path.read_bytes()
        path.write_bytes(raw.replace(b"immutable",b"different",1))
        with self.assertRaisesRegex(ValueError,"SHA256"):
            history.load(self.root,binding)
        path.write_bytes(raw)
        binding["sha256"]="0"*64
        with self.assertRaisesRegex(ValueError,"SHA256"):
            history.load(self.root,binding)

    def test_array_rebinding_cannot_replace_audited_original_text(self):
        binding=self.authority()
        def read(name):return json.loads((self.root/("history/"+name+".json")).read_bytes())
        rows=read("B1-warmup.results")
        rows[0]["text"]="different authorized-looking text"
        arraybinding=self.write("history/B1-warmup.results.json",rows)
        inputs=read("input-bindings")
        inputs["bindings"]=[arraybinding if x["path"]==arraybinding["path"] else x for x in inputs["bindings"]]
        inputbinding=self.write("history/input-bindings.json",inputs)
        numeric=read("numeric");numeric["inputBindings"]=inputbinding
        numericbinding=self.write("history/numeric.json",numeric)
        report=read("report");report["bindings"]["assembledInputs"]=inputbinding;report["bindings"]["independentNumericReview"]=numericbinding
        reportbinding=self.write("history/report.json",report)
        approval=read("approval");approval["qualityReport"]=reportbinding;approval["independentNumericReview"]=numericbinding
        binding=self.write("history/approval.json",approval)
        with self.assertRaisesRegex(ValueError,"canonical"):
            history.load(self.root,binding)

    def test_completed_history_still_requires_numeric_and_semantic_authority(self):
        binding=self.authority()
        approval=json.loads((self.root/binding["path"]).read_bytes())
        semantic=json.loads((self.root/approval["semanticReview"]["path"]).read_bytes())
        semantic["semanticNewErrorAssessment"]["newWrongNumberObserved"]=True
        new=self.write(approval["semanticReview"]["path"],semantic)
        report=json.loads((self.root/approval["qualityReport"]["path"]).read_bytes())
        report["bindings"]["independentSemanticReview"]=new
        approval["semanticReview"]=new;approval["qualityReport"]=self.write(approval["qualityReport"]["path"],report)
        with self.assertRaisesRegex(ValueError,"semantic"):
            history.load(self.root,self.write(binding["path"],approval))


    def test_existing_fingerprint_plan_rejects_rewritten_same_bytes(self):
        binding=self.authority()
        fingerprints={}
        history.load(self.root,binding,fingerprints)
        path=self.root/"history/B1-warmup.results.json"
        data=path.read_bytes()
        path.write_bytes(data)
        with self.assertRaisesRegex(ValueError,"changed"):
            history.load(self.root,binding,fingerprints)


if __name__ == "__main__":
    unittest.main()
