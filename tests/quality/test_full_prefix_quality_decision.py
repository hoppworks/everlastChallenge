import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("full_prefix_quality_decision", ROOT / "scripts/screening_quality/decision.py")
decision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(decision)


class ApprovalTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.runtime = {"syntheticDecoderPolicy": "fixed"}
        self.policy = {"candidate": True, "syntheticPolicy": "full-prefix"}
        self.sources = []
        pins = []
        for i in range(47):
            binding = self.write("sources/pin-%02d.rs" % i, "synthetic source %d" % i)
            self.sources.append(binding)
            pins.append(dict(filename=binding["path"], sha256=binding["sha256"]))
        self.parameters = {}
        quality_parameters = {}
        for candidate, role in ((False,"reference"),(True,"candidate")):
            parameter = dict(runtime=self.runtime, fullPrefix=dict(self.policy,candidate=candidate), sources=pins)
            self.parameters[role] = copy.deepcopy(parameter)
            quality_parameters["FullPrefix" + ("Candidate" if candidate else "Reference")] = self.write(role+"-parameters.json",parameter)
        self.freeze = dict(kind="prospective-full-prefix-screening-v1", schemaVersion=1,
            screeningId="synthetic-screen", sourceRevision="a"*40, qualified=False,L=None,
            parameters=quality_parameters,sources=self.sources)
        self.completion = dict(kind="full-prefix-screening-execution-v1",schemaVersion=1,
            screeningId="synthetic-screen",sourceRevision="a"*40,completed=True,error=None,
            rowCount=800,powerAssertionEnded=True,qualified=False,L=None,blocks=[])
        for i,variant in enumerate(("FullPrefixReference","FullPrefixCandidate","FullPrefixCandidate","FullPrefixReference")):
            cases = [dict(id="case-%03d" % j,status="ok",error=None,
                rows=[dict(status="ok",validated=True,warmup=w) for w in (True,False)]) for j in range(100)]
            self.completion["blocks"].append(dict(name="B%d"%(i+1),variant=variant,cases=cases))
        self.freeze["blocks"] = [dict(name=b["name"],variant=b["variant"],cases=[dict(id=c["id"]) for c in b["cases"]])
                                 for b in self.completion["blocks"]]
        self.report = dict(schemaVersion=1,kind="full-prefix-quality-screening-decision-v1",
            sourceRevision="a"*40,screeningId="synthetic-screen",qualified=False,L=None,
            decision=dict(numericalScreenPassed=True,candidateRejected=False,
                          productionFullPrefixEnabled=False,performanceRunStarted=False))
        self.approval = dict(schemaVersion=1,kind="full-prefix-quality-decision-v1",passed=True,
            decision="eligible-for-latency-measurement",productionActivated=False,
            runtime=self.runtime,candidateFullPrefix=self.policy,
            semanticReview=self.write("semantic.json",dict(synthetic=True,review="external authority fixture")))

    def write(self, name, value):
        path = self.root/name
        path.parent.mkdir(parents=True,exist_ok=True)
        data = value.encode() if isinstance(value,str) else (json.dumps(value)+"\n").encode()
        path.write_bytes(data)
        os.chmod(path,0o600)
        return dict(path=name,bytes=len(data),mode="0o600",sha256=hashlib.sha256(data).hexdigest())

    def binding(self):
        self.approval["screeningFreeze"] = self.write("freeze.json",self.freeze)
        self.completion["freezeSha256"] = self.approval["screeningFreeze"]["sha256"]
        self.approval["screeningCompletion"] = self.write("completion.json",self.completion)
        self.approval["qualityReport"] = self.write("report.json",self.report)
        return self.write("decision.json",self.approval)

    def test_external_bound_approval_allows_additional_wrapper_with_identical_quality_sources(self):
        # All inputs are synthetic authority fixtures, never actual qualification.
        extra = self.write("new-batch-wrapper.rs","new wrapper")
        for p in self.parameters.values():
            p["sources"].append(dict(filename=extra["path"],sha256=extra["sha256"]))
        checked = decision.validate(self.root,self.binding(),{},self.parameters)
        self.assertEqual(checked["screeningId"],"synthetic-screen")
        self.assertEqual(checked["compiledQualitySourcesChecked"],47)

    def test_candidate_only_remainder_uses_same_bound_quality_approval(self):
        checked = decision.validate(self.root,self.binding(),{},self.parameters["candidate"])
        self.assertEqual(checked["compiledQualitySourcesChecked"],47)

    def test_rebound_completion_cannot_replace_a_frozen_case(self):
        self.completion["blocks"][-1]["cases"][0]["id"] = "different-fixture"
        with self.assertRaises(ValueError):
            decision.validate(self.root,self.binding(),{},self.parameters)

    def test_approval_numerical_completion_and_parameter_mutations_reject(self):
        mutations = [
            lambda:self.approval.update(passed=False),
            lambda:self.approval.update(kind="pretranscription-quality-decision-v1"),
            lambda:self.approval.update(productionActivated=True),
            lambda:self.approval.update(runtime={"different":True}),
            lambda:self.approval.update(candidateFullPrefix=dict(self.policy,syntheticPolicy="suffix")),
            lambda:self.report["decision"].update(numericalScreenPassed=False),
            lambda:self.report["decision"].update(candidateRejected=True),
            lambda:self.report["decision"].update(performanceRunStarted=True),
            lambda:self.report.update(screeningId="foreign-screen"),
            lambda:self.completion.update(completed=False),
            lambda:self.completion.update(powerAssertionEnded=False),
            lambda:self.completion.update(rowCount=799),
            lambda:self.completion["blocks"][1]["cases"][50]["rows"][1].update(validated=False),
            lambda:self.completion["blocks"][2]["cases"][40].update(status="unavailable"),
            lambda:self.parameters["candidate"]["runtime"].update(syntheticDecoderPolicy="changed"),
            lambda:self.parameters["candidate"]["sources"][0].update(sha256="0"*64),
            lambda:self.freeze["sources"].append(copy.deepcopy(self.sources[0])),
        ]
        original = copy.deepcopy((self.approval,self.report,self.completion,self.parameters,self.freeze))
        for index,mutate in enumerate(mutations):
            self.approval,self.report,self.completion,self.parameters,self.freeze = copy.deepcopy(original)
            mutate()
            with self.subTest(mutation=index),self.assertRaises(ValueError):
                decision.validate(self.root,self.binding(),{},self.parameters)

    def test_bound_source_semantic_review_and_approval_bytes_must_remain_unchanged(self):
        for role in ("source","semantic","approval"):
            binding = self.binding()
            path = self.root/({"source":self.sources[0]["path"],"semantic":self.approval["semanticReview"]["path"],
                               "approval":binding["path"]}[role])
            previous = path.read_bytes()
            path.write_bytes(b"x"*len(previous))
            with self.subTest(role=role),self.assertRaises(ValueError):
                decision.validate(self.root,binding,{},self.parameters)
            path.write_bytes(previous)


if __name__ == "__main__":
    unittest.main()
