"""Load approved historical reference text bytes, never new timing or approval.

The caller supplies an independently trusted old approval binding. This module
checks that bound evidence, not current source equivalence. Any new comparison
still needs a separate, bound reference-algorithm equivalence authority.
"""
import hashlib
import json
from pathlib import Path
import re

from . import decision

common = decision.common
check = common.check
SOURCE_REVISION = "301fee4cea4d9c87023bf949838d3468ecba2e95"
MANIFEST_SHA256 = "df432b44c48a39ebe290c6bd20d9ba8b800fd2211cdd898c86a9b22a54edf030"
MODEL_BINDINGS = {
    "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2": 574041195,
    "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf": 885098,
}
PHASES = ("warmup", "measured")


def _digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def _same_binding(left, right):
    check(decision.same(left, right), "historical evidence binding differs")


def load(root, approval_binding, fingerprints=None):
    """Return four original UTF-8 JSON array bytes and original manifest bytes.

    No model, audio, individual process log, or live source is opened. Returned
    provenance names only the old source; no inferred equivalence or new run.
    File fingerprints join the caller's optional unchanged-file plan.
    """
    root = Path(root).resolve(strict=True)
    fingerprints = {} if fingerprints is None else fingerprints

    def raw(binding):
        common.check_unchanged({"fingerprints": fingerprints})
        return common.verify_binding(root, binding, fingerprints, common.MAX_JSON, True)

    def read(binding):
        return common.parse_json(raw(binding))

    approval = read(approval_binding)
    check(type(approval["schemaVersion"]) is int and approval["schemaVersion"] == 1 and
          approval["kind"] == "full-prefix-quality-decision-v1" and approval["passed"] is True and
          approval["decision"] == "eligible-for-latency-measurement" and approval["productionActivated"] is False,
          "trusted historical quality approval required")
    freeze = read(approval["screeningFreeze"])
    completion = read(approval["screeningCompletion"])
    report = read(approval["qualityReport"])
    numeric = read(approval["independentNumericReview"])
    semantic = read(approval["semanticReview"])
    check(freeze["kind"] == "prospective-full-prefix-screening-v1" and
          freeze["sourceRevision"] == SOURCE_REVISION and freeze["qualified"] is False and freeze["L"] is None,
          "historical source identity differs")
    check(completion["kind"] == "full-prefix-screening-execution-v1" and completion["completed"] is True and
          completion["error"] is None and type(completion["rowCount"]) is int and completion["rowCount"] == 800 and
          completion["powerAssertionEnded"] is True and completion["freezeSha256"] == approval["screeningFreeze"]["sha256"],
          "historical completion is not complete")
    for document in (completion, report, semantic):
        check(document["sourceRevision"] == SOURCE_REVISION and document["screeningId"] == freeze["screeningId"],
              "historical source or screening identity differs")
    check(report["kind"] == "full-prefix-quality-screening-decision-v1" and
          report["decision"]["numericalScreenPassed"] is True and report["decision"]["semanticScreenPassed"] is True and
          report["decision"]["candidateRejected"] is False and report["decision"]["productionFullPrefixEnabled"] is False,
          "historical numerical/semantic decision does not pass")
    for key, authority in (("screeningFreeze", "screeningFreeze"), ("screeningCompletion", "screeningCompletion"),
                           ("independentNumericReview", "independentNumericReview"), ("independentSemanticReview", "semanticReview")):
        _same_binding(report["bindings"][key], approval[authority])
    evaluation = report["canonicalEvaluation"]
    check(evaluation["unicodeVersion"] == "13.0.0" and evaluation["pythonVersion"] == "3.9.6" and
          type(evaluation["separateReports"]) is int and evaluation["separateReports"] == 8 and
          type(evaluation["casesPerReport"]) is int and evaluation["casesPerReport"] == 100,
          "historical canonical evaluator differs")
    check(numeric["kind"] == "independent-full-prefix-canonical-numeric-readback-v1" and
          numeric["auditPassed"] is True and numeric["numericalScreenPassed"] is True and
          numeric["canonicalUnicodeVersion"] == "13.0.0" and numeric["casesEach"] == 100 and
          numeric["all800RawTextValuesCopiedExactly"] is True and numeric["eightCanonicalCliExit0Recorded"] is True,
          "historical independent numeric evidence incomplete")
    _same_binding(numeric["freeze"], approval["screeningFreeze"])
    _same_binding(semantic["bindings"]["freeze"], approval["screeningFreeze"])
    _same_binding(semantic["bindings"]["globalCompletion"], approval["screeningCompletion"])
    assessment = semantic["semanticNewErrorAssessment"]
    check(semantic["kind"] == "full-prefix-completed-semantic-review-v1" and
          semantic["summary"]["caseSlots"] == 400 and semantic["summary"]["rows"] == 800 and
          semantic["summary"]["distinctGoldCases"] == 100 and assessment["completePairedPhaseCoverage"] is True and
          all(assessment[k] is False for k in ("newLostNegationObserved", "newWrongNumberObserved",
              "newSegmentOmissionObserved", "newSegmentDuplicationObserved", "newAnchorFailureObserved")),
          "historical semantic evidence incomplete")
    check(len(freeze["models"]) == 2 and
          {b["sha256"]: b["bytes"] for b in freeze["models"]} == MODEL_BINDINGS,
          "historical model/VAD identity differs")
    check(set(freeze["parameters"]) == set(decision.VARIANTS), "historical parameter roles differ")
    parameters = {role: read(binding) for role, binding in freeze["parameters"].items()}
    pins = None
    for role, parameter in parameters.items():
        check(parameter["fullPrefix"]["candidate"] is (role == "FullPrefixCandidate") and
              decision.same(parameter["runtime"], approval["runtime"]), "historical parameter role/policy differs")
        current = common.compiled_sources(parameter)
        check(pins is None or pins == current, "historical source arms differ")
        pins = current
    source_bindings = {b["path"]: b for b in freeze["sources"]}
    check(len(source_bindings) == len(freeze["sources"]) and len(pins) == 47 and
          all(source_bindings[n]["sha256"] == sha and re.fullmatch("[0-9a-f]{64}", sha) for n,sha in pins.items()),
          "historical compiled-source bindings incomplete")
    check(len(freeze["blocks"]) == len(completion["blocks"]) == 4 and
          tuple(b["variant"] for b in freeze["blocks"]) == tuple(b["variant"] for b in completion["blocks"]) == decision.VARIANTS,
          "historical block role differs")
    inputs_binding = report["bindings"]["assembledInputs"]
    _same_binding(inputs_binding, numeric["inputBindings"])
    _same_binding(report["bindings"]["assemblyOrigin"], numeric["origin"])
    inputs = read(inputs_binding)
    origin = read(numeric["origin"])
    check(inputs["kind"] == "complete-eight-quality-inputs" and origin["kind"] == "full-prefix-quality-assembly-v1" and
          origin["unicodeVersion"] == "13.0.0", "historical assembly identity differs")
    for key, authority in (("freeze", "screeningFreeze"), ("completion", "screeningCompletion")):
        _same_binding(origin["authorityBindings"][key], approval[authority])
    _same_binding(origin["authorityBindings"]["parameters"], freeze["parameters"])
    descriptors = {b["path"]: b for b in inputs["bindings"]}
    check(len(descriptors) == len(inputs["bindings"]) == 10, "historical canonical inputs incomplete")
    check(descriptors.get(numeric["origin"]["path"]) == numeric["origin"] and
          descriptors.get(numeric["manifest"]["path"]) == numeric["manifest"], "historical input bindings differ")
    manifest_bytes = raw(numeric["manifest"])
    manifest = common.parse_json(manifest_bytes)
    check(numeric["manifest"]["sha256"] == freeze["manifest"]["sha256"] == MANIFEST_SHA256 and
          numeric["manifest"]["bytes"] == freeze["manifest"]["bytes"] and
          manifest["unicodeVersion"] == "13.0.0" and len(manifest["cases"]) == 100,
          "fixed historical manifest differs")
    ids = [c["id"] for c in manifest["cases"]]
    check(len(set(ids)) == 100, "historical manifest IDs repeat")
    for i, (old, done) in enumerate(zip(freeze["blocks"], completion["blocks"])):
        check(old["name"] == done["name"] == "B%d" % (i+1) and
              [c["id"] for c in old["cases"]] == [c["id"] for c in done["cases"]] == ids,
              "historical block IDs incomplete or reordered")
        check(all(c["status"] == "ok" and c["error"] is None and len(c["rows"]) == 2 and
                  all(row["status"] == "ok" and row["validated"] is True and row["warmup"] is warm
                      for row,warm in zip(c["rows"], (True,False))) for c in done["cases"]),
              "historical rows not valid")
    check(len(origin["inputs"]) == 8 and len(origin["slots"]) == 800, "historical assembly coverage differs")
    arrays = []
    for block in ("B1", "B4"):
        bindex = int(block[1])-1
        for pindex, phase in enumerate(PHASES):
            candidates = [b for path,b in descriptors.items() if path.endswith("/"+block+"-"+phase+".results.json")]
            check(len(candidates) == 1, "historical reference array binding missing or repeated")
            data = raw(candidates[0]); rows = common.parse_json(data)
            check(type(rows) is list and len(rows) == 100 and [row["id"] for row in rows] == ids and
                  all(set(row) == {"id","status","text"} and row["status"] == "ok" and type(row["text"]) is str for row in rows),
                  "historical reference array IDs/status/text differ")
            entry = origin["inputs"][bindex*2+pindex]
            check(entry == dict(block=block,phase=phase,rows=100,canonicalSha256=_digest(rows)), "historical array canonical binding differs")
            for index, (row, case) in enumerate(zip(rows, manifest["cases"])):
                slot = origin["slots"][bindex*200+index*2+pindex]
                check(slot["slot"] == bindex*100+index+1 and slot["block"] == block and slot["variant"] == "FullPrefixReference" and
                      slot["caseIndex"] == index+1 and slot["id"] == row["id"] and slot["phase"] == phase,
                      "historical reference slot role/identity differs")
                check(slot["origin"] == dict(sourceRevision=SOURCE_REVISION,screeningId=freeze["screeningId"],
                      freezeSha256=approval["screeningFreeze"]["sha256"],completionPath=freeze["blocks"][bindex]["cases"][index]["completion"]),
                      "historical reference origin differs")
                check(slot["rawRunId"] == ("warmup" if pindex == 0 else "measured-001") and
                      slot["referenceUtf8Sha256"] == hashlib.sha256(case["reference"].encode()).hexdigest() and
                      slot["sourceRawRowCanonicalSha256"] == _digest(dict(row,id=slot["rawRunId"],warmup=pindex == 0)),
                      "historical reference text/gold/phase binding differs")
            arrays.append((block, phase, data))
    common.check_unchanged({"fingerprints": fingerprints})
    return dict(reference_arrays=tuple(arrays),manifest_bytes=manifest_bytes,source_revision=SOURCE_REVISION,
                provenance=dict(approval=approval_binding,freeze=approval["screeningFreeze"],completion=approval["screeningCompletion"],
                    numeric=approval["independentNumericReview"],semantic=approval["semanticReview"],models=freeze["models"],
                    parameters=freeze["parameters"],compiled_sources=[source_bindings[n] for n in sorted(pins)],
                    arrays=[b for b in descriptors.values() if any(b["path"].endswith("/"+block+"-"+phase+".results.json")
                            for block in ("B1","B4") for phase in PHASES)]),
                reuse_authorized=False,requires_bound_external_reference_equivalence=True,
                new_candidate_approval=None,new_timing=None)
