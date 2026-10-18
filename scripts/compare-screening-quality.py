#!/usr/bin/env python3
"""ABBA comparison of canonical Unicode-13 numeric reports, without text normalization.

The eight inputs are labelled by their prospective phase/block roles. Their raw
provenance, full text/anchors, pipeline and P06 qualification remain separate.
"""
import importlib.util
import math
import json
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location("paced_numeric_io", Path(__file__).with_name("run-paced-remainder.py"))
io = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(io)
SELECTION_SHA = "864332ed3e11c4924066f920ff8c98e2c2c98f6040ddf0bff53ceff3b40551a9"
DENOMINATORS_SHA = "9868ffc7bd44128c7223ac241140de5b80fa98f4421ee104dcf809da3264037a"
SELECTION_VALUE_SHA = "d6d26d6caeb87d33aa0f62c57f1a898d6aff7adab4ccf50b8ba265d94708d582"
DENOMINATORS_VALUE_SHA = "8781da5f87c3e0f771d65815c1c8ae7d5167613399f9f21417f2dc77682083d0"
DENOMINATORS = Path(__file__).resolve().parents[1]/"tests/fixtures/corpus/pretranscription-quality-denominators-v1.json"
PHASES = ("warmup", "measured")
ROLES = {(phase, block) for phase in PHASES for block in (1, 2, 3, 4)}
ORIGINAL = "tests/fixtures/corpus/acceptance-v1.json"
CASE_FIELDS = {"id","split","language","class","status","edits","refWords","hypWords","wer","unwantedText"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def count(value):
    return type(value) is int and 0 <= value <= 1_000_000


def ratio_matches(actual, expected):
    return actual is None if expected is None else (
        type(actual) in (int,float) and math.isfinite(actual) and actual == expected)


def aggregate(rows):
    groups = {}
    for row in rows:
        identity = (row["split"],row["language"],row["class"])
        group = groups.setdefault(identity, dict(split=identity[0],language=identity[1],**{"class":identity[2]},
            caseCount=0,scoredCount=0,failedCount=0,missingCount=0,silenceCount=0,
            silenceFailedCount=0,unwantedTextCount=0,expectedRefWords=0,refWords=0,edits=0))
        empty, ok = row["refWords"] == 0, row["status"] == "ok"
        group["caseCount"] += 1
        group["expectedRefWords"] += row["refWords"]
        group["failedCount"] += not ok
        group["missingCount"] += row["status"] == "missing"
        group["silenceCount"] += empty
        group["silenceFailedCount"] += empty and not ok
        group["unwantedTextCount"] += row["unwantedText"] is True
        if row["edits"] is not None:
            group["scoredCount"] += 1
            group["refWords"] += row["refWords"]
            group["edits"] += row["edits"]
    for group in groups.values():
        group["complete"] = group["failedCount"] == 0
        group["observedWer"] = group["edits"]/group["refWords"] if group["refWords"] else None
        group["wer"] = group["observedWer"] if group["complete"] else None
    return groups


def validate_report(selection, report):
    require(set(report) == {"schemaVersion","unicodeVersion","cases","groups"} and
            type(report["schemaVersion"]) is int and report["schemaVersion"] == 1 and
            report["unicodeVersion"] == "13.0.0", "canonical numeric report schema differs")
    require(isinstance(report["cases"],list) and len(report["cases"]) == 100, "incomplete100-case report")
    by_id = {}
    expected = {case["id"]:case for case in selection["cases"]}
    for row in report["cases"]:
        require(isinstance(row,dict) and set(row) == CASE_FIELDS, "unsupported numeric case fields")
        ident = row["id"]
        require(isinstance(ident,str) and ident in expected and ident not in by_id, "duplicate/unknown report case ID")
        case = expected[ident]
        require(all(row[k] == case[k] for k in ("language","class","split")), "case language/class/split changed")
        require(row["status"] in ("ok","failed","missing") and count(row["refWords"]) and
                row["refWords"] <= 16384, "invalid case status/denominator")
        n, hyp, edits = row["refWords"],row["hypWords"],row["edits"]
        require((n == 0) == (case["expectedOutcome"] == "Empty"), "frozen Empty/Local denominator differs")
        require(hyp is None or (count(hyp) and hyp <= 16384), "invalid hypothesis count")
        if row["status"] == "ok":
            require(hyp is not None, "successful case lacks hypothesis count")
        if row["status"] == "missing":
            require(hyp is None, "missing case claims hypothesis text")
        if n and row["status"] == "ok":
            require(count(edits) and abs(n-hyp) <= edits <= max(n,hyp) and
                    ratio_matches(row["wer"],edits/n), "invalid edit count/case WER")
        else:
            require(edits is None and row["wer"] is None, "failed/empty case claims WER")
        if n:
            require(row["unwantedText"] is False, "spoken case has invalid silence flag")
        else:
            # Any token entails raw non-whitespace output. Zero tokens can
            # still represent unwanted punctuation, so do not infer false.
            if hyp is None:
                require(row["unwantedText"] is None, "absent silence text claims a raw-output judgement")
            else:
                require(type(row["unwantedText"]) is bool and
                        (hyp == 0 or row["unwantedText"] is True),
                        "silence token count contradicts raw unwantedText")
        by_id[ident] = row
    groups = aggregate(by_id.values())
    require(isinstance(report["groups"],list) and len(report["groups"]) == len(groups), "group coverage differs")
    seen = set()
    for actual in report["groups"]:
        require(isinstance(actual,dict), "invalid numeric group")
        identity = tuple(actual.get(k) for k in ("split","language","class"))
        require(identity in groups and identity not in seen, "duplicate/unknown numeric group")
        seen.add(identity)
        wanted = groups[identity]
        require(set(actual) == set(wanted), "numeric group fields differ")
        for field,value in wanted.items():
            if field in ("wer","observedWer"):
                require(ratio_matches(actual[field],value), "group WER differs from case totals")
            elif type(value) is int:
                require(type(actual[field]) is int and actual[field] == value, "group count differs from case totals")
            elif type(value) is bool:
                require(type(actual[field]) is bool and actual[field] == value, "group completeness differs")
            else:
                require(actual[field] == value, "group identity differs")
    return by_id


def value_sha(value):
    return io.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False,allow_nan=False).encode())


def compare_counts(reference_edits, candidate_edits, reference_words, clean):
    """Exact arithmetic for one complete original-corpus group; no rounding."""
    require(all(count(v) for v in (reference_edits,candidate_edits,reference_words)) and reference_words>0
            and type(clean) is bool, "invalid complete group counts")
    delta = 200*(candidate_edits-reference_edits) <= reference_words
    absolute = not clean or 10*candidate_edits <= reference_words
    return dict(deltaWithinHalfPercentagePoint=delta,candidateWithinAbsoluteCleanLimit=absolute,passed=delta and absolute)


def compare(selection, reports, denominators=None):
    require(value_sha(selection) == SELECTION_VALUE_SHA, "fixed selection value binding differs")
    if denominators is None:
        raw = io.bounded_bytes(DENOMINATORS,io.MAX_JSON)
        require(io.sha256(raw) == DENOMINATORS_SHA, "independent denominator fixture hash differs")
        denominators = io.parse_json(raw)
    require(value_sha(denominators) == DENOMINATORS_VALUE_SHA, "independent denominator fixture value binding differs")
    require(isinstance(selection,dict) and isinstance(selection.get("cases"),list) and len(selection["cases"]) == 100,
            "complete fixed100 selection required")
    cases = selection["cases"]
    require(len({c["id"] for c in cases}) == 100 and sum(c["sourceManifest"] == ORIGINAL for c in cases) == 90,
            "selection requires90 original and10 separate cases")
    require(set(reports) == ROLES, "exactly eight labelled phase/block reports required")
    validated = {role:validate_report(selection,report) for role,report in reports.items()}
    base = validated[("measured",1)]
    expected_words = {row["id"]:row for row in denominators["cases"]}
    require(len(expected_words) == 100, "incomplete independent word-count authority")
    for case in cases:
        expected = expected_words[case["id"]]
        require(all(case[k] == expected[k] for k in ("id","language","class","split","referenceUtf8Sha256")) and
                expected["original90"] is (case["sourceManifest"] == ORIGINAL), "reference identity differs from independent count authority")
        require(all(rows[case["id"]]["refWords"] == expected["refWords"] for rows in validated.values()),
                "reference denominator differs from independent Unicode13 count authority")
    require(all(rows[ident]["refWords"] == row["refWords"] for rows in validated.values()
                for ident,row in base.items()), "per-case denominators differ between reports")
    original_ids = [c["id"] for c in cases if c["sourceManifest"] == ORIGINAL]
    supplement_ids = [c["id"] for c in cases if c["sourceManifest"] != ORIGINAL]
    phases = []
    all_passed = True
    for phase in PHASES:
        pairs = []
        for reference,candidate in ((1,2),(4,3)):
            rr,cr = validated[(phase,reference)],validated[(phase,candidate)]
            groups = []
            for language in ("de","en"):
                for classification in ("clean","challenging"):
                    ids = [ident for ident in original_ids if rr[ident]["language"] == language and rr[ident]["class"] == classification]
                    require(bool(ids), "original language/class group missing")
                    n = sum(rr[ident]["refWords"] for ident in ids)
                    complete = all(rows[ident]["status"] == "ok" for rows in (rr,cr) for ident in ids)
                    re = sum(rr[ident]["edits"] or 0 for ident in ids)
                    ce = sum(cr[ident]["edits"] or 0 for ident in ids)
                    gates = compare_counts(re,ce,n,classification=="clean") if complete else dict(
                        deltaWithinHalfPercentagePoint=False,candidateWithinAbsoluteCleanLimit=False,passed=False)
                    delta_ok = gates["deltaWithinHalfPercentagePoint"]
                    absolute = gates["candidateWithinAbsoluteCleanLimit"]
                    passed = gates["passed"]
                    groups.append(dict(language=language,**{"class":classification},caseCount=len(ids),
                        referenceTokens=n,referenceEdits=re if complete else None,candidateEdits=ce if complete else None,
                        complete=complete,deltaWithinHalfPercentagePoint=delta_ok,candidateWithinAbsoluteCleanLimit=absolute,passed=passed))
            failures = [dict(id=ident,block=block,status=validated[(phase,block)][ident]["status"])
                        for block in (reference,candidate) for ident in base if validated[(phase,block)][ident]["status"] != "ok"]
            unwanted = [dict(id=ident,block=block) for block in (reference,candidate) for ident in base
                        if validated[(phase,block)][ident]["unwantedText"] is True]
            passed = all(g["passed"] for g in groups) and not failures and not unwanted
            all_passed &= passed
            extras = []
            for ident in supplement_ids:
                extras.append(dict(id=ident,language=rr[ident]["language"],**{"class":rr[ident]["class"]},
                    reference={k:rr[ident][k] for k in ("status","refWords","hypWords","edits","unwantedText")},
                    candidate={k:cr[ident][k] for k in ("status","refWords","hypWords","edits","unwantedText")}))
            pairs.append(dict(referenceBlock=reference,candidateBlock=candidate,original90=groups,
                supplement10=extras,failedCases=failures,unwantedSilence=unwanted,numericalPassed=passed))
        phases.append(dict(phase=phase,pairs=pairs))
    return dict(schemaVersion=1,kind="pretranscription-screening-numeric-comparison",
        numericalScreenPassed=bool(all_passed),phases=phases,qualified=False,L=None,
        independentDenominators=dict(sha256=DENOMINATORS_SHA,sourceReport=denominators["sourceReport"],
            manifest=denominators["manifest"],selection=denominators["selection"],method=denominators["method"]),
        limits=["numeric screening only; no full text, anchor or segment judgement",
                "pipeline/source and phase/block provenance require independent validation",
                "no Unicode normalization performed; inputs must be canonical Unicode13 reports",
                "per-case denominators match the pinned historical Unicode13 count authority; no normalization is rerun",
                "no latency, resource or P06 qualification"])


def main(argv=None):
    import argparse
    import sys
    parser=argparse.ArgumentParser(description=__doc__,
        epilog="Eight report paths in block order: B1_WARMUP B1_MEASURED B2_WARMUP B2_MEASURED B3_WARMUP B3_MEASURED B4_WARMUP B4_MEASURED. Numerical success never qualifies P06.")
    parser.add_argument("selection",type=Path)
    parser.add_argument("reports",type=Path,nargs=8)
    parser.add_argument("--denominators",type=Path,default=DENOMINATORS)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args(argv)
    try:
        sources=[args.selection,args.denominators,*args.reports]
        require(len({p.resolve() for p in args.reports})==8,"eight distinct phase/block report files required")
        require(args.output.resolve() not in {p.resolve() for p in sources},"output aliases an input")
        selection_bytes=io.bounded_bytes(args.selection,io.MAX_JSON)
        require(io.sha256(selection_bytes)==SELECTION_SHA,"fixed selection byte binding differs")
        denominator_bytes=io.bounded_bytes(args.denominators,io.MAX_JSON)
        require(io.sha256(denominator_bytes)==DENOMINATORS_SHA,"independent denominator fixture byte binding differs")
        selected=io.parse_json(selection_bytes)
        denominators=io.parse_json(denominator_bytes)
        reports,bindings={},[]
        for block in (1,2,3,4):
            for index,phase in enumerate(PHASES):
                path=args.reports[(block-1)*2+index]
                data=io.bounded_bytes(path,io.MAX_JSON)
                reports[(phase,block)]=io.parse_json(data)
                bindings.append(dict(phase=phase,block=block,
                    variant="PretransReference" if block in (1,4) else "PretransCandidate",
                    path=str(path),bytes=len(data),sha256=io.sha256(data)))
        compared=compare(selected,reports,denominators)
        compared["sourceReports"]=bindings
        compared["selectionBinding"]=dict(path=str(args.selection),bytes=len(selection_bytes),sha256=SELECTION_SHA)
        compared["denominatorBinding"]=dict(path=str(args.denominators),bytes=len(denominator_bytes),sha256=DENOMINATORS_SHA)
        evaluator=Path(__file__).with_name("quality.py")
        evaluator_bytes=io.bounded_bytes(evaluator,io.MAX_JSON)
        require(io.sha256(evaluator_bytes)==denominators["method"]["evaluator"]["sha256"],"canonical evaluator source binding differs")
        compared["comparisonSources"]=[dict(path=str(path),sha256=io.sha256(io.bounded_bytes(path,io.MAX_JSON)))
            for path in (Path(__file__),Path(io.__file__),evaluator)]
        io.write_private(args.output,compared)
        print("screening-quality: numeric comparison complete; qualified=false")
        return 0 if compared["numericalScreenPassed"] else 1
    except (ValueError,OSError,KeyError,TypeError,AttributeError,RecursionError,OverflowError):
        # Fixed message; never echo source content or exceptions containing it.
        print("screening-quality: invalid, incomplete, unbound input or unavailable fresh output",file=sys.stderr)
        return 2


if __name__=="__main__":
    raise SystemExit(main())
