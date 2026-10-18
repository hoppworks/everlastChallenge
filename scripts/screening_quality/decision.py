"""Bound external FullPrefix quality approval, never a semantic evaluator.

The approval is written only after actual numerical and independent semantic
readback. Its bound semantic review is an external authority, not inferred from
text equality. A new performance wrapper may differ; the 47 compiled quality
sources and decoder policy must remain exactly equal.
"""
import importlib.util
import json
from pathlib import Path
import re

_SPEC = importlib.util.spec_from_file_location("quality_decision_files", Path(__file__).resolve().parents[1] / "run-paced-remainder.py")
common = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(common)
check = common.check
VARIANTS = ("FullPrefixReference","FullPrefixCandidate","FullPrefixCandidate","FullPrefixReference")


def same(left, right):
    return json.dumps(left,sort_keys=True,separators=(",",":"),allow_nan=False) == json.dumps(
        right,sort_keys=True,separators=(",",":"),allow_nan=False)


def validate(root, binding, fingerprints, parameters):
    """Validate an external approval and source equivalence; run no evaluator.

    Parameters are a candidate-only document or a pair keyed by caller roles.
    The external passed decision covers its bound semantic review. This module
    neither judges names/anchors nor derives that decision from a numerical gate.
    All checked bindings join the caller's unchanged-file fingerprint plan.
    """
    root = Path(root).resolve(strict=True)

    def load(descriptor, maximum=common.MAX_JSON):
        return common.parse_json(common.verify_binding(root,descriptor,fingerprints,maximum,True))

    approval = load(binding)
    check(type(approval["schemaVersion"]) is int and approval["schemaVersion"] == 1 and
          approval["kind"] == "full-prefix-quality-decision-v1" and approval["passed"] is True and
          approval["decision"] == "eligible-for-latency-measurement" and approval["productionActivated"] is False,
          "external full-prefix quality approval required")
    check(approval["semanticReview"]["bytes"] > 0,"bound semantic review missing")
    common.verify_binding(root,approval["semanticReview"],fingerprints,common.MAX_TEXT)
    freeze = load(approval["screeningFreeze"])
    completion = load(approval["screeningCompletion"],common.MAX_METADATA)
    report = load(approval["qualityReport"],common.MAX_METADATA)
    check(type(freeze["schemaVersion"]) is int and freeze["schemaVersion"] == 1 and
          freeze["kind"] == "prospective-full-prefix-screening-v1" and
          freeze["qualified"] is False and freeze["L"] is None and
          re.fullmatch("[0-9a-f]{40}",freeze["sourceRevision"]) is not None,
          "unknown prospective quality authority")
    check(type(completion["schemaVersion"]) is int and completion["schemaVersion"] == 1 and
          completion["kind"] == "full-prefix-screening-execution-v1" and completion["completed"] is True and
          completion["error"] is None and type(completion["rowCount"]) is int and completion["rowCount"] == 800 and
          completion["powerAssertionEnded"] is True and completion["qualified"] is False and completion["L"] is None,
          "full400/800 quality execution incomplete")
    check(completion["freezeSha256"] == approval["screeningFreeze"]["sha256"] and
          completion["sourceRevision"] == report["sourceRevision"] == freeze["sourceRevision"] and
          completion["screeningId"] == report["screeningId"] == freeze["screeningId"],
          "quality authority identity differs")
    check(type(report["schemaVersion"]) is int and report["schemaVersion"] == 1 and
          report["kind"] == "full-prefix-quality-screening-decision-v1" and report["qualified"] is False and
          report["L"] is None and report["decision"]["numericalScreenPassed"] is True and
          report["decision"]["candidateRejected"] is False and
          report["decision"]["productionFullPrefixEnabled"] is False and
          report["decision"]["performanceRunStarted"] is False,"bound numerical quality decision does not pass")
    check(len(completion["blocks"]) == len(freeze["blocks"]) == 4 and
          tuple(b["variant"] for b in completion["blocks"]) == tuple(b["variant"] for b in freeze["blocks"]) == VARIANTS,
          "complete ABBA quality comparison required")
    for block,frozen in zip(completion["blocks"],freeze["blocks"]):
        check(block["name"] == frozen["name"] and len(block["cases"]) == 100 and
              len({case["id"] for case in block["cases"]}) == 100 and
              [case["id"] for case in block["cases"]] == [case["id"] for case in frozen["cases"]],
              "quality block membership incomplete or repeated")
        for case in block["cases"]:
            check(case["status"] == "ok" and case["error"] is None and len(case["rows"]) == 2 and
                  all(row["status"] == "ok" and row["validated"] is True and row["warmup"] is warmup
                      for row,warmup in zip(case["rows"],(True,False))),"quality execution has invalid rows")
    documents = [parameters] if "fullPrefix" in parameters else list(parameters.values())
    check(len(documents) in (1,2) and all(type(p["fullPrefix"]["candidate"]) is bool for p in documents),
          "explicit performance parameter arms required")
    arms = {p["fullPrefix"]["candidate"]:p for p in documents}
    check(len(arms) == len(documents) and True in arms,"candidate-only or matched performance pair required")
    check(all(same(approval["runtime"],p["runtime"]) for p in documents) and
          same(approval["candidateFullPrefix"],arms[True]["fullPrefix"]),"approved decoder/candidate policy differs")
    check(set(freeze["parameters"]) == set(VARIANTS),"quality parameter arms differ")
    quality_parameters = {role:load(descriptor) for role,descriptor in freeze["parameters"].items()}
    pins = None
    for role,document in quality_parameters.items():
        candidate = role == "FullPrefixCandidate"
        check(document["fullPrefix"]["candidate"] is candidate and
              same(document["runtime"],approval["runtime"]) and
              same(document["fullPrefix"],dict(approval["candidateFullPrefix"],candidate=candidate)) and
              (candidate not in arms or same(document["fullPrefix"],arms[candidate]["fullPrefix"])),
              "performance policy differs from quality execution")
        source_pins = common.compiled_sources(document)
        check(pins is None or source_pins == pins,"quality source arms conflict")
        pins = source_pins
    check(len(pins) == 47,"complete compiled quality source closure required")
    sources = {source["path"]:source for source in freeze["sources"]}
    check(len(sources) == len(freeze["sources"]),"duplicate quality source descriptor")
    performance_pins = [common.compiled_sources(document) for document in documents]
    for name,digest in pins.items():
        check(sources[name]["sha256"] == digest and all(p.get(name) == digest for p in performance_pins),
              "shared quality algorithm source changed")
        common.verify_binding(root,sources[name],fingerprints,common.MAX_JSON)
    return dict(screeningId=freeze["screeningId"],qualitySourceRevision=freeze["sourceRevision"],
                compiledQualitySourcesChecked=len(pins),decisionSha256=binding["sha256"])
