#!/usr/bin/env python3
"""Replay the unchanged 94 cases for the frozen MLX encoder winner only."""
import argparse
import hashlib
import importlib.util
import io
import json
import math
import os
from pathlib import Path
import signal
import stat
import statistics
import subprocess
import unicodedata
import wave

SPEC = importlib.util.spec_from_file_location("remainder_contract", Path(__file__).with_name("run-paced-remainder.py"))
common = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(common)
MODEL_ID = "large-v3-turbo-reference-q5-coefficients-mlx-f32"
MODEL_SHA = "277a2661c6d85f7e620adad07f7fda78b052a5a66b2c16e9fb6ccb3ac134d042"
MODEL_BYTES = 3235574296
DECODER_MODEL_SHA = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"
BACKEND = "mlxrs-encoder+whisper.cpp-decoder"
CACHE_CENTRAL_KIND = "native-mlx-f16-cache256-pair-projection-v1"
CACHE_REMAINDER_KIND = "native-mlx-reference-encoder-cache256-remainder-v1"


def reviewed_quality(manifest, canonical, baseline, candidate, reviews):
    """Admit the original 90+10 full cases through the unchanged review oracle."""
    for document in (manifest, canonical):
        common.check(document["schemaVersion"] == 1 and document["unicodeVersion"] == "13.0.0",
                     "quality manifest schema/Unicode differs")
    cases = manifest["cases"]
    indexed = {case["id"]: case for case in cases}
    canonical_cases = canonical["cases"]
    canonical_ids = {case["id"] for case in canonical_cases}
    common.check(len(cases) == len(indexed) == 100 and len(canonical_cases) == len(canonical_ids) == 90,
                 "complete distinct 100/90 quality cases required")
    common.check(all(indexed.get(case["id"]) == case for case in canonical_cases),
                 "canonical full quality object differs")
    common.check(len(baseline) == len(candidate) == 100
                 and {row["id"] for row in baseline} == {row["id"] for row in candidate} == set(indexed),
                 "complete original baseline/candidate required")
    spec = importlib.util.spec_from_file_location("mlx_reviewed_quality", Path(__file__).with_name("compare_mlx_quality.py"))
    compare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(compare)
    for selected in (canonical_cases, [case for case in cases if case["id"] not in canonical_ids]):
        ids = {case["id"] for case in selected}
        result = compare.compare(selected, [row for row in baseline if row["id"] in ids],
                                 [row for row in candidate if row["id"] in ids], reviews)
        common.check(result["qualified"], "quality or exact complete-object review incomplete")
    return indexed


def command_for(plan, case, paths):
    command = ["/usr/bin/time", "-l", str(plan["binary"]), str(plan["root"] / plan["model"]["path"]),
               "mlx-reference-encoder", str(plan["root"] / case["audio"]["path"]), case["language"],
               str(plan["parameterPath"]), str(paths["results"]), "10"]
    if case["expectedOutcome"] == "Empty":
        command.append("--expect-empty")
    return command


def validate_case(plan, case, paths):
    result = common.validate_case(plan, case, paths)
    if result["errors"]:
        return result
    try:
        metadata = common.read_json(paths["metadata"], common.MAX_METADATA)
        common.check(metadata["backend"] == BACKEND, "actual hybrid backend identity missing")
        common.check(metadata["contextCleanup"] is None and metadata["pretranscriptionCandidate"] is None
                     and metadata["fullPrefixCandidate"] is None, "unexpected runtime experiment")
        if plan.get("encoderPrecision") == "fp16":
            runtime = metadata["parameters"]["runtime"]
            common.check(runtime["encoderDtype"] == "fp16"
                         and runtime["encoderCheckpointDtype"] == runtime["encoderFeatureDtype"] == "fp32",
                         "F16 compute with unchanged F32 checkpoint/bridge required")
            events = [json.loads(line) for line in common.bounded_bytes(paths["log"], common.MAX_JSON)
                      .decode("utf-8").splitlines() if line.startswith("{")]
            loaded = [event for event in events if event.get("kind") == "mlx-reference-encoder-loaded"]
            common.check(len(loaded) == 1 and loaded[0]["encoderComputeDtype"] == "fp16"
                         and loaded[0]["checkpointDtype"] == loaded[0]["bridgeFeatureDtype"] == "fp32"
                         and loaded[0]["checkpointSha256"] == MODEL_SHA
                         and loaded[0]["decoderModelSha256"] == DECODER_MODEL_SHA,
                         "actual loaded F16 encoder identity missing or different")
            if plan.get("cacheBudgetBytes") is not None:
                installed = [event for event in events if event.get("kind") == "mlx-cache-budget-installed"]
                common.check(plan["cacheBudgetBytes"] == 268435456 and len(installed) == 1
                             and installed[0].get("bytes") == plan["cacheBudgetBytes"],
                             "actual selected 256 MiB cache budget receipt missing or different")
    except (OSError, ValueError, KeyError, TypeError) as error:
        result["errors"].append(str(error) if isinstance(error, ValueError) else "hybrid metadata missing")
    return result


def binding(path, root):
    info = os.stat(path, follow_symlinks=False)
    common.check(stat.S_ISREG(info.st_mode), "binding must be a regular file")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    common.check(common.fingerprint(info) == common.fingerprint(os.stat(path, follow_symlinks=False)), "binding changed")
    return {"path": str(path.relative_to(root)), "bytes": info.st_size,
            "mode": oct(stat.S_IMODE(info.st_mode)), "sha256": digest.hexdigest()}


def verify(root, record, fingerprints, maximum=common.MAX_JSON, read=False):
    """Bounded streaming hash; the derived F32 model deliberately exceeds 1 GiB."""
    path = common.relative_path(root, record["path"])
    common.check(type(record["bytes"]) is int and 0 <= record["bytes"] <= maximum, "binding exceeds bound")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    digest = hashlib.sha256()
    chunks = []
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        common.check(stat.S_ISREG(before.st_mode) and before.st_size == record["bytes"]
                     and oct(stat.S_IMODE(before.st_mode)) == record["mode"], "binding size/type/mode differs")
        remaining = record["bytes"]
        while remaining:
            chunk = stream.read(min(1024 * 1024, remaining))
            common.check(bool(chunk), "binding truncated")
            remaining -= len(chunk)
            digest.update(chunk)
            if read:
                chunks.append(chunk)
        common.check(not stream.read(1) and common.fingerprint(before) == common.fingerprint(os.fstat(stream.fileno())),
                     "binding changed while read")
    common.check(digest.hexdigest() == record["sha256"], "binding SHA differs")
    fingerprints[path] = common.fingerprint(before)
    return b"".join(chunks) if read else None


def validate_cache_projection(root, freeze, central, completed, readback, fingerprints):
    """Bind the replay projection to the original accepted six-arm experiment."""
    common.check(central.get("kind") == CACHE_CENTRAL_KIND and central.get("projectionOnly") is True
                 and central.get("cacheBudgetBytes") == freeze.get("cacheBudgetBytes") == 268435456,
                 "qualified cache projection identity missing")
    verify(root, central["projectionBuilder"], fingerprints)
    bindings = central["cacheProvenance"]
    evidence = {key: common.parse_json(verify(root, record, fingerprints, read=True))
                for key, record in bindings.items()}
    build, policy = evidence["buildFreeze"], evidence["cachePolicy"]
    original, done, accepted = (evidence[key] for key in
                                ("sixArmFreeze", "sixArmCompletion", "sixArmAcceptance"))
    common.check(original["kind"] == done["kind"] == central["sourceKind"]
                 == "cache256-CPP-unbounded-mirrored-six-arm-v1"
                 and original["noRetries"] is True and done["observedRows"] == 1086,
                 "actual fixed six-arm experiment required")
    common.check(done["freezeSha256"] == accepted["freezeSha256"] == bindings["sixArmFreeze"]["sha256"]
                 and accepted["completionSha256"] == bindings["sixArmCompletion"]["sha256"]
                 == completed["sourceSixArmCompletionSha256"], "six-arm proof belongs to another experiment")
    common.check(all(done.get(key) is True for key in ("completed", "candidateEvidenceSufficient",
                 "semanticReviewComplete", "powerGatePassed", "caffeinateReaped"))
                 and all(accepted.get(key) is True for key in ("allChecksPassed", "bothPairOrdersComplete",
                         "candidatePaceEvidenceSufficient", "semanticReviewComplete", "powerGatePassed"))
                 and accepted["completedArms"] == 6 and accepted["observedRows"] == 1086,
                 "complete independent six-arm admission required")
    common.check(accepted["independentReadbackSha256"] == freeze["centralReadback"]["sha256"]
                 and readback["bindings"]["freezeSha256"] == bindings["sixArmFreeze"]["sha256"]
                 and readback["bindings"]["completionSha256"] == bindings["sixArmCompletion"]["sha256"]
                 and readback["allIndependentChecksPassed"] is True,
                 "detailed current independent readback binding differs")
    arm_labels = {"01-cpp", "02-cache256", "03-unbounded", "04-unbounded", "05-cache256", "06-cpp"}
    common.check(set(accepted["arms"]) == set(readback["arms"]) == arm_labels
                 and all(accepted["arms"][label] == readback["arms"][label]["bindings"] for label in arm_labels),
                 "all six raw arm witnesses must match the independent readback")
    pairs = done["comparisons"]["cacheVsCpp"]
    common.check([(pair["beforeArm"], pair["afterArm"]) for pair in pairs]
                 == [("01-cpp", "02-cache256"), ("06-cpp", "05-cache256")], "both actual cache/CPP orders required")
    for key in ("candidateAbsoluteLimitsPassed", "allCaseMediansImproved", "allCaseP95Improved"):
        common.check(completed[key] is True and all(pair[key] is True for pair in pairs),
                     "projected case gate differs from actual paired evidence")
    common.check(policy["selectedCacheBudgetBytes"] == 268435456
                 and policy["buildFreezeSha256"] == bindings["buildFreeze"]["sha256"]
                 and str(root / bindings["buildFreeze"]["path"]) == original["profiles"]["cache256"]["buildFreeze"]
                 and build["encoderComputeDtype"] == "fp16"
                 and build["sourceManifestSha256"] == central["sourceManifestSha256"]
                 and build["kernelSha256"] == central["metallibSha256"]
                 and build["models"] == central["models"] == original["models"],
                 "replay build/model/cache policy differs from the measured profile")
    runtime_binary = next(record for record in build["binaries"] if record["name"] == "paced_runtime_probe")
    central_binary = next(record for record in build["binaries"] if record["name"] == "paced_reference_probe")
    common.check(runtime_binary["sha256"] == freeze["binary"]["sha256"]
                 and runtime_binary["features"] == central["binaryRecords"][0]["features"]
                 and central_binary["sha256"] == original["profiles"]["cache256"]["binarySha256"],
                 "runtime and measured binaries must come from the same accepted build")
    measured_parameter = original["parameters"]["cache256"]
    measured_path = common.relative_path(root, bindings["sixArmFreeze"]["path"]).parent / measured_parameter["file"]
    measured_info = os.stat(measured_path, follow_symlinks=False)
    measured_record = {"path": str(measured_path.relative_to(root)), "bytes": measured_info.st_size,
                       "mode": oct(stat.S_IMODE(measured_info.st_mode)), "sha256": measured_parameter["sha256"]}
    measured_parameters = common.parse_json(verify(root, measured_record, fingerprints, read=True))
    replay_parameters = common.parse_json(verify(root, freeze["parameters"], fingerprints, read=True))
    common.check(replay_parameters["runtime"] == measured_parameters["runtime"],
                 "replay runtime/decoder parameters differ from the measured cache policy")
    for key, record_key, independent_key in (
        ("quality100Passed", "qualityAcceptance", "qualityIndependent"),
        ("prospective12Passed", "prospectiveAcceptance", "prospectiveIndependent"),
    ):
        admission = accepted["admission"][key]
        record = evidence[record_key]
        common.check(record[key] is True and record["allExecutionChecksPassed"] is True
                     and record["semanticReviewComplete"] is True
                     and admission["acceptanceSha256"] == bindings[record_key]["sha256"],
                     "current quality/fresh admission differs from six-arm evidence")
        independent_binding = freeze[independent_key] if key == "quality100Passed" else bindings[independent_key]
        common.check(admission["independentReadbackSha256"] == independent_binding["sha256"],
                     "independent quality/fresh readback differs")
    common.check(bindings["qualityAcceptance"]["sha256"] == freeze["qualityAcceptance"]["sha256"]
                 and accepted["admission"]["quality100Passed"]["completionSha256"] == freeze["qualityCompletion"]["sha256"]
                 and bindings["prospectiveCompletion"]["sha256"] == evidence["prospectiveAcceptance"]["completionSha256"]
                 == accepted["admission"]["prospective12Passed"]["completionSha256"]
                 and bindings["prospectiveCandidate"]["sha256"] == evidence["prospectiveAcceptance"]["candidateSha256"]
                 and evidence["prospectiveCompletion"]["completed"] is True
                 and evidence["prospectiveIndependent"]["allIndependentChecksPassed"] is True,
                 "complete fresh candidate evidence missing or changed")
    stage = common.relative_path(root, bindings["sixArmCompletion"]["path"]).parent
    for label, record in accepted["arms"].items():
        meta_path = stage / (label + ".json.metadata.json")
        before = os.stat(meta_path, follow_symlinks=False)
        common.check(stat.S_ISREG(before.st_mode)
                     and list(common.fingerprint(before)[:5]) == record["metadataFingerprint"],
                     "read-back raw metadata changed")
        fingerprints[meta_path] = common.fingerprint(before)
        for path, digest in ((stage / (label + ".json"), record["rawSha256"]),
                             (stage / label / "owner-completion.json", record["ownerSha256"])):
            info = os.stat(path, follow_symlinks=False)
            bound = {"path": str(path.relative_to(root)), "bytes": info.st_size,
                     "mode": oct(stat.S_IMODE(info.st_mode)), "sha256": digest}
            verify(root, bound, fingerprints)
        owner = common.read_json(stage / label / "owner-completion.json", common.MAX_JSON)
        common.check(owner["exitCode"] == 0 and owner["terminationConfirmed"] is True
                     and owner["processReaped"] is True and not owner["forcedGroupCleanup"]
                     and not owner["watchdogExpired"] and not owner["cancelled"], "six-arm owner not cleanly ended")
        try:
            os.killpg(owner["wrapperPid"], 0)
        except ProcessLookupError:
            continue
        common.check(False, "previous model group still present")


def prepare(root, central, quality, freeze_path, output, encoder_precision="fp32"):
    """Freeze only after the complete ABBA comparison; no measurement starts here."""
    common.check(encoder_precision in ("fp32", "fp16"), "unsupported encoder precision")
    central_freeze = common.read_json(central / "freeze.json", common.MAX_JSON)
    abba = common.read_json(central / "completion.json", common.MAX_JSON)
    cache = "cacheBudgetBytes" in central_freeze
    if cache:
        common.check(encoder_precision == "fp16" and central_freeze["cacheBudgetBytes"] == 268435456,
                     "selected cache profile requires explicit FP16")
    for key in ("completed", "semanticReviewComplete", "candidateAbsoluteLimitsPassed", "allCaseMediansImproved"):
        common.check(abba.get(key) is True, "central ABBA winner gate incomplete: " + key)
    if encoder_precision == "fp16":
        expected_kind = CACHE_CENTRAL_KIND if cache else "native-mlx-f16-ordered-pairs-v2"
        common.check(abba.get("kind") == expected_kind, "bound F16 ordered-pair evidence required")
        for key in ("bothPairOrdersComplete", "allCaseP95Improved", "powerGatePassed"):
            common.check(abba.get(key) is True, "F16 central evidence incomplete: " + key)
    common.check(abba["freezeSha256"] == binding(central / "freeze.json", root)["sha256"], "ABBA freeze differs")
    common.check(not freeze_path.exists(), "freeze must be new")
    params = central / central_freeze["parameters"]["candidate-runtime"]["file"]
    selection = root / "tests/fixtures/corpus/remainder-timing-selection-v1.json"
    cases = common.read_json(selection, common.MAX_JSON)["cases"]
    records = {"schemaVersion": 1, "kind": "native-mlx-reference-encoder-remainder-v1",
               "centralFreeze": binding(central / "freeze.json", root),
               "centralCompletion": binding(central / "completion.json", root),
               "sourceManifest": binding(central / "sources.json", root),
               "binary": binding(central / "bin/paced_runtime_probe", root),
               "metallib": binding(central / "bin/mlx.metallib", root),
               "parameters": binding(params, root), "selection": binding(selection, root),
               "models": [binding(root / item["path"], root) for item in central_freeze["models"]],
               "inputs": [binding(root / case["audio"]["path"], root) for case in cases],
               "reference": binding(quality / "reference.json", root),
               "qualityCandidate": binding(quality / "candidate.json", root),
               "qualityCompletion": binding(quality / "completion.json", root),
               "runnerSources": [binding(path, root) for path in (Path(__file__).resolve(),
                  Path(common.__file__).resolve(), root / "scripts/compare_mlx_quality.py",
                  root / "scripts/quality.py", root / "tests/quality/test_mlx_paced_remainder.py")],
               "outputDirectory": str(output.relative_to(root)), "warmupsPerCase": 1,
               "measuredPerCase": 10, "caseTimeoutSeconds": common.WATCHDOG_SECONDS,
               "qualified": False, "L": None, "noRetries": True}
    if encoder_precision == "fp16":
        records.update(kind=CACHE_REMAINDER_KIND if cache else "native-mlx-reference-encoder-f16-remainder-v1",
                       encoderPrecision="fp16")
        if cache:
            records["cacheBudgetBytes"] = 268435456
        for key, path in {
            "centralReadback": central / "independent-readback.json",
            "qualityManifest": quality / "manifest.json", "qualityCanonical": quality / "canonical.json",
            "qualityReviews": quality / "semantic-reviews.json", "qualityAcceptance": quality / "acceptance-readback.json",
            "qualityOwner": quality / "owner-completion.json", "qualityMetadata": quality / "candidate.json.metadata.json",
            "qualityIndependent": (common.relative_path(root, central_freeze["independentQualityReadbackPath"]) if cache
                                   else root / "artifacts/implementation/mlx-f16-runtime-quality100-readback.json"),
        }.items():
            records[key] = binding(path, root)
        records["runnerSources"] += [binding(root / name, root) for name in (
            "scripts/owned_probe.py", "tests/quality/test_owned_probe.py", "tests/quality/test_mlx_f16_paced_remainder.py")]
        if cache:
            records["runnerSources"].append(binding(root / "tests/quality/test_mlx_cache_paced_remainder.py", root))
    common.write_private(freeze_path, records)


def preflight(root, freeze_path):
    common.check(unicodedata.unidata_version == "13.0.0", "frozen Unicode 13 evaluator runtime required before launch")
    freeze_bytes = common.bounded_bytes(freeze_path, common.MAX_JSON)
    freeze = common.parse_json(freeze_bytes)
    cache = freeze.get("kind") == CACHE_REMAINDER_KIND
    f16 = cache or freeze.get("kind") == "native-mlx-reference-encoder-f16-remainder-v1"
    common.check(freeze["schemaVersion"] == 1 and (f16 or freeze["kind"] == "native-mlx-reference-encoder-remainder-v1"),
                 "freeze kind/schema differs")
    if f16:
        common.check(freeze.get("encoderPrecision") == "fp16", "explicit F16 compute identity required")
    common.check(cache or "cacheBudgetBytes" not in freeze, "cache budget requires its independently admitted profile")
    common.check(freeze["warmupsPerCase"] == 1 and freeze["measuredPerCase"] == 10
                 and freeze["caseTimeoutSeconds"] == common.WATCHDOG_SECONDS and freeze["noRetries"] is True,
                 "fixed replay protocol differs")
    fingerprints = {}
    def parsed(record):
        return common.parse_json(verify(root, record, fingerprints, read=True))
    central = parsed(freeze["centralFreeze"])
    completed = parsed(freeze["centralCompletion"])
    common.check(completed["freezeSha256"] == freeze["centralFreeze"]["sha256"], "central completion belongs to another freeze")
    for key in ("completed", "semanticReviewComplete", "candidateAbsoluteLimitsPassed", "allCaseMediansImproved"):
        common.check(completed.get(key) is True, "central winner incomplete: " + key)
    if f16:
        expected_kind = CACHE_CENTRAL_KIND if cache else "native-mlx-f16-ordered-pairs-v2"
        common.check(completed.get("kind") == expected_kind, "bound F16 ordered-pair evidence required")
        for key in ("bothPairOrdersComplete", "allCaseP95Improved", "powerGatePassed"):
            common.check(completed.get(key) is True, "F16 central evidence incomplete: " + key)
        central_readback = parsed(freeze["centralReadback"])
        common.check(freeze["centralReadback"]["sha256"] == completed["independentReadbackSha256"]
                     and central_readback["allChecksPassed"] is True
                     and central_readback["bothPairOrdersComplete"] is True,
                     "independent complete two-order readback required")
        if cache:
            validate_cache_projection(root, freeze, central, completed, central_readback, fingerprints)
    source_rows = parsed(freeze["sourceManifest"])
    common.check(freeze["sourceManifest"]["sha256"] == central["sourceManifestSha256"], "central source manifest differs")
    sources = {item["path"]: item for item in source_rows}
    common.check(len(sources) == len(source_rows), "duplicate frozen source")
    parameters = parsed(freeze["parameters"])
    common.check(freeze["parameters"]["sha256"] == central["parameters"]["candidate-runtime"]["sha256"], "central parameters differ")
    common.check(parameters["configurationId"] == "native-mlx-reference-encoder-paced-v1"
                 and parameters["build"]["debugAssertions"] is False, "release hybrid configuration required")
    common.check(parameters["runtime"]["fullParams"]["no_timestamps"] is False
                 and parameters["runtime"]["fullParams"]["suppress_nst"] is False, "reference decoding policy differs")
    if f16:
        runtime = parameters["runtime"]
        common.check(runtime["encoderDtype"] == "fp16"
                     and runtime["encoderCheckpointDtype"] == runtime["encoderFeatureDtype"] == "fp32",
                     "F16 compute with F32 checkpoint/bridge required")
    clock = parameters["measurement"]
    common.check(clock["publisherThread"]["policy"] == "timeConstrained", "TC publisher required")
    for key, value in {"chunkMillis": 20, "jitterToleranceNanos": 10_000_000,
                       "maximumSleepNanos": 1_000_000, "finalSpinNanos": 250_000}.items():
        common.check(clock[key] == value, "fixed replay clock differs")
    source_folder = common.relative_path(root, freeze["sourceManifest"]["path"]).parent / "source"
    for name, digest in common.compiled_sources(parameters).items():
        common.check(name in sources and sources[name]["sha256"] == digest, "compiled source pin differs")
        record = dict(sources[name], path=str((source_folder / name).relative_to(root)),
                      mode=oct(stat.S_IMODE(os.stat(source_folder / name, follow_symlinks=False).st_mode)))
        verify(root, record, fingerprints)
    expected_binary = next(item for item in central["binaryRecords"] if item["file"] == "paced_runtime_probe")
    if f16:
        common.check("mlx-encoder-f16" in expected_binary["features"], "F16 compiled feature missing")
    common.check(freeze["binary"]["sha256"] == expected_binary["sha256"]
                 and freeze["metallib"]["sha256"] == central["metallibSha256"], "native artifacts differ")
    common.check(Path(freeze["binary"]["path"]).parent == Path(freeze["metallib"]["path"]).parent,
                 "kernel must be colocated with probe")
    for record in (freeze["binary"], freeze["metallib"]):
        verify(root, record, fingerprints, 512 * 1024 * 1024)
    required_runners = {"scripts/run-mlx-paced-remainder.py", "scripts/run-paced-remainder.py",
                        "scripts/compare_mlx_quality.py", "scripts/quality.py", "tests/quality/test_mlx_paced_remainder.py"}
    if f16:
        required_runners.update(("scripts/owned_probe.py", "tests/quality/test_owned_probe.py",
                                 "tests/quality/test_mlx_f16_paced_remainder.py"))
    if cache:
        required_runners.add("tests/quality/test_mlx_cache_paced_remainder.py")
    common.check({row["path"] for row in freeze["runnerSources"]} == required_runners
                 and len(freeze["runnerSources"]) == len(required_runners), "runner/oracle sources missing or duplicated")
    for record in freeze["runnerSources"]:
        verify(root, record, fingerprints)
    assets = {item["file"]: item for item in parameters["models"]}
    common.check(len(assets) == len(freeze["models"]) == 6
                 and {Path(item["path"]).name for item in freeze["models"]} == set(assets), "all six distinct model assets required")
    parents = set()
    for record in freeze["models"]:
        asset = assets[Path(record["path"]).name]
        common.check(all(record[key] == asset[key] for key in ("bytes", "sha256")), "model asset differs")
        parents.add(Path(record["path"]).parent)
        verify(root, record, fingerprints, 4 * 1024 * 1024 * 1024)
    common.check(len(parents) == 1, "model assets must be siblings")
    model = next(item for item in freeze["models"] if Path(item["path"]).name == "weights.safetensors")
    common.check(model["bytes"] == MODEL_BYTES and model["sha256"] == MODEL_SHA, "qualified coefficient identity differs")
    model = dict(model, id=MODEL_ID)
    common.check(freeze["selection"]["sha256"] == common.SELECTION_SHA, "unchanged 94-case selection required")
    selection = parsed(freeze["selection"])
    cases = selection["cases"]
    common.check(len(cases) == len({c["id"] for c in cases}) == 94, "all 94 unique cases required")
    manifests = {}
    for record in selection["sourceManifests"] + [selection["centralManifest"]]:
        path = common.relative_path(root, record["path"])
        manifests[record["path"]] = parsed(dict(record, mode=oct(stat.S_IMODE(path.stat().st_mode))))
    inputs = {item["path"]: item for item in freeze["inputs"]}
    common.check(len(inputs) == len(freeze["inputs"]) == 94, "all input bindings required")
    references = []
    for case in cases:
        common.check(case["warmupRuns"] == 1 and case["measuredRuns"] == 10 and case["language"] in ("de", "en"), "case protocol differs")
        source = [c for c in manifests[case["sourceManifest"]]["cases"] if c["id"] == case["id"]]
        common.check(len(source) == 1 and all(source[0][key] == case[key] for key in ("audio", "language", "class", "split")),
                     "source case differs")
        reference = source[0]["reference"]
        common.check(common.sha256(reference.encode()) == case["referenceUtf8Sha256"], "independent reference differs")
        common.check(case["expectedOutcome"] == ("Local" if reference.strip() else "Empty"), "case outcome differs")
        audio = case["audio"]
        record = inputs[audio["path"]]
        common.check(record["sha256"] == audio["sha256"] and 0 < audio["frames"] <= 4_800_000, "input identity/limit differs")
        data = verify(root, record, fingerprints, 10 * 1024 * 1024, True)
        with wave.open(io.BytesIO(data), "rb") as wav:
            common.check((wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes(), wav.getcomptype())
                         == (1, 2, 16000, audio["frames"], "NONE"), "PCM header differs")
            common.check(len(wav.readframes(audio["frames"] + 1)) == audio["frames"] * 2, "PCM length differs")
        references.append({key: source[0][key] for key in ("id", "language", "class", "split", "reference")})
    common.check(sum(c["expectedOutcome"] == "Empty" for c in cases) == 6, "six Empty cases required")
    baseline, candidate, quality = (parsed(freeze[key]) for key in ("reference", "qualityCandidate", "qualityCompletion"))
    extras = {}
    if f16:
        evidence = {key: parsed(freeze[key]) for key in ("qualityManifest", "qualityCanonical", "qualityReviews",
                    "qualityAcceptance", "qualityOwner", "qualityMetadata", "qualityIndependent")}
        for key, central_key in (("reference", "qualityReferenceSha256"), ("qualityCandidate", "qualityCandidateSha256"),
                                 ("qualityReviews", "qualityReviewsSha256"), ("qualityAcceptance", "qualityAcceptanceSha256"),
                                 ("qualityIndependent", "independentQualityReadbackSha256")):
            common.check(freeze[key]["sha256"] == central[central_key], "F16 quality input differs from central freeze: " + key)
        for key, source in (("qualityManifest", "tests/fixtures/corpus/p06-context-quality-manifest-v1.json"),
                            ("qualityCanonical", "tests/fixtures/corpus/acceptance-v1.json")):
            common.check(freeze[key]["sha256"] == sources[source]["sha256"], "frozen full quality manifest differs")
        accepted = evidence["qualityAcceptance"]
        common.check(all(accepted.get(key) is True for key in ("quality100Passed", "allExecutionChecksPassed",
                     "allQualityGroupsPassed", "semanticReviewComplete")), "reviewed quality acceptance incomplete")
        for key, accepted_key in (("qualityCandidate", "candidateSha256"), ("qualityCompletion", "completionSha256"),
                                  ("qualityReviews", "semanticReviewsSha256"), ("qualityOwner", "ownerCompletionSha256")):
            common.check(freeze[key]["sha256"] == accepted[accepted_key], "quality acceptance belongs to another object")
        common.check(quality["completed"] is True and quality["completedCases"] == 100
                     and quality["automaticGatePassed"] is True
                     and quality["metadataSha256"] == freeze["qualityMetadata"]["sha256"]
                     and quality["candidateSha256"] == freeze["qualityCandidate"]["sha256"]
                     and quality["ownerCompletionSha256"] == freeze["qualityOwner"]["sha256"], "raw full quality execution differs")
        owner = evidence["qualityOwner"]
        common.check(owner["exitCode"] == 0 and owner["processReaped"] is True and owner["terminationConfirmed"] is True
                     and not owner["forcedGroupCleanup"] and not owner["cancelled"] and not owner["watchdogExpired"],
                     "quality owner not cleanly ended")
        common.check(evidence["qualityIndependent"]["allIndependentChecksPassed"] is True,
                     "independent full quality readback missing")
        metadata = evidence["qualityMetadata"]
        common.check(metadata["engine"]["encoderDtype"] == "fp16"
                     and all(metadata[key] is None for key in ("loadError", "serviceError", "cleanupError"))
                     and len(metadata["cases"]) == 100
                     and [row["id"] for row in metadata["cases"]] == [row["id"] for row in candidate]
                     and all(row["status"] == row["executionStatus"] == "ok" and row["error"] is None
                             for row in metadata["cases"]), "complete actual F16 quality metadata required")
        loaded = quality["loadedDtypeReceipt"]
        common.check(loaded["encoderComputeDtype"] == "fp16"
                     and loaded["checkpointDtype"] == loaded["bridgeFeatureDtype"] == "fp32"
                     and loaded["checkpointSha256"] == MODEL_SHA and loaded["decoderModelSha256"] == DECODER_MODEL_SHA,
                     "actual F16 full quality receipt missing")
        full_cases = reviewed_quality(evidence["qualityManifest"], evidence["qualityCanonical"], baseline, candidate,
                                      evidence["qualityReviews"])
        for case, reference in zip(cases, references):
            full = full_cases[case["id"]]
            common.check(all(full[key] == case[key] for key in ("audio", "language", "class", "split"))
                         and full["reference"] == reference["reference"], "replay case differs from approved full quality case")
        extras = {"encoderPrecision": "fp16", "qualityCases": full_cases, "semanticReviews": evidence["qualityReviews"]}
        if cache:
            extras["cacheBudgetBytes"] = freeze["cacheBudgetBytes"]
    else:
        common.check(len(baseline) == len(candidate) == 100 and baseline == candidate
                     and quality["qualityPassed"] is True and quality["semanticReviewComplete"] is True,
                     "frozen complete quality evidence differs")
    baseline = {row["id"]: row for row in baseline}
    common.check(set(c["id"] for c in cases).issubset(baseline), "quality baseline does not cover remainder")
    output = common.relative_path(root, freeze["outputDirectory"])
    common.check(Path(freeze["outputDirectory"]).parts[0] == "artifacts" and not output.exists()
                 and not output.is_symlink() and output.parent.is_dir(), "output must be new under artifacts")
    return {"root": root, "output": output, "cases": cases, "parameters": parameters,
            "parameterSha256": freeze["parameters"]["sha256"], "sourceRevision": freeze["centralFreeze"]["sha256"],
            "model": model, "binary": common.relative_path(root, freeze["binary"]["path"]),
            "parameterPath": common.relative_path(root, freeze["parameters"]["path"]),
            "fingerprints": fingerprints, "references": references, "baseline": baseline,
            "freezeSha256": common.sha256(freeze_bytes), "freezeBytes": freeze_bytes, **extras}


def reap_f16(process):
    spec = importlib.util.spec_from_file_location("mlx_remainder_owned_probe", Path(__file__).with_name("owned_probe.py"))
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    return owner.reap_owned(process)


def launch_case(plan, case, paths):
    command = command_for(plan, case, paths)
    result = {"command": command, "startedUtc": common.utc_now(), "finishedUtc": None, "wrapperPid": None,
              "exitCode": None, "launchError": None, "watchdogExpired": False, "terminationConfirmed": False}
    process = None
    try:
        descriptor = os.open(paths["log"], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as log:
            process = subprocess.Popen(command, cwd=plan["root"], start_new_session=True, stdout=log, stderr=subprocess.STDOUT,
                env={**os.environ, "EVERLAST_BENCHMARK_REVISION": plan["sourceRevision"], "EVERLAST_MLX_SEGMENT_DIAGNOSTICS": "1"})
            common._ACTIVE_PROCESS = process
            result["wrapperPid"] = process.pid
            print(json.dumps({"caseId": case["id"], "wrapperPid": process.pid, "startedUtc": result["startedUtc"]}), flush=True)
            try:
                if common._INTERRUPTED:
                    raise KeyboardInterrupt
                result["exitCode"] = process.wait(timeout=common.WATCHDOG_SECONDS)
                result["terminationConfirmed"] = True
                common._ACTIVE_PROCESS = None
                if plan.get("encoderPrecision") == "fp16":
                    result.update(reap_f16(process))
                    if not result["terminationConfirmed"] or result["forcedGroupCleanup"]:
                        result["launchError"] = "owned F16 probe group not cleanly confirmed absent"
            except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
                common._ACTIVE_PROCESS = None
                result["watchdogExpired"] = isinstance(error, subprocess.TimeoutExpired)
                result["launchError"] = "owned replay watchdog expired" if result["watchdogExpired"] else "runner interrupted"
                if plan.get("encoderPrecision") == "fp16":
                    result.update(reap_f16(process))
                else:
                    result["exitCode"], result["terminationConfirmed"] = common.stop_owned_group(process)
        if result["exitCode"] == 0:
            errors = validate_case(plan, case, paths)["errors"]
            if errors:
                result["launchError"] = "; ".join(([result["launchError"]] if result["launchError"] else []) + errors)
    except OSError:
        result["launchError"] = "owned wrapper launch/observation failed"
        if process is not None:
            if plan.get("encoderPrecision") == "fp16":
                result.update(reap_f16(process))
            else:
                result["exitCode"], result["terminationConfirmed"] = common.stop_owned_group(process)
        else:
            result["terminationConfirmed"] = True
    finally:
        common._ACTIVE_PROCESS = None
        result["finishedUtc"] = common.utc_now()
    return result


def stats(values):
    ordered = sorted(values)
    return {"count": len(values), "medianNanos": statistics.median(ordered),
            "p95Nanos": ordered[math.ceil(.95 * len(ordered)) - 1], "minNanos": ordered[0], "maxNanos": ordered[-1]}


def readback(plan, report):
    common.check(report["completed"] is True and len(report["cases"]) == 94, "all cases must complete before readback")
    common.check_unchanged(plan)
    spec = importlib.util.spec_from_file_location("mlx_remainder_quality", Path(__file__).with_name("compare_mlx_quality.py"))
    compare = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(compare)
    rounds = [{"references": [], "before": [], "after": []} for _ in range(11)]
    f16 = plan.get("encoderPrecision") == "fp16"
    measurements = []
    by_id = {row["id"]: row for row in plan["references"]}
    for index, case in enumerate(plan["cases"]):
        paths = common.paths_for(plan["output"], index, case)
        validated = validate_case(plan, case, paths)
        common.check(not validated["errors"], "complete readback validation failed")
        rows = common.read_json(paths["results"], common.MAX_TEXT)
        for repetition, row in enumerate(rows):
            identifier = case["id"] if f16 else case["id"] + "::" + row["id"]
            selected = rounds[repetition]
            selected["references"].append(plan["qualityCases"][case["id"]] if f16
                                           else dict(by_id[case["id"]], id=identifier))
            baseline = plan["baseline"][case["id"]]
            selected["before"].append({"id": identifier, "status": baseline["status"], "text": baseline["text"]})
            selected["after"].append({"id": identifier, "status": row["status"], "text": row["text"]})
        measured = [row for row in validated["rows"] if not row["warmup"]]
        measurements.append({"id": case["id"], "language": case["language"], "class": case["class"],
             "expectedOutcome": case["expectedOutcome"], "terminal": stats([r["stopToTerminalNanos"] for r in measured]),
             "text": None if case["expectedOutcome"] == "Empty" else stats([r["stopToTextNanos"] for r in measured]),
             "engine": stats([r["engineElapsedNanos"] for r in measured])})
    # The unchanged oracle bounds each evaluation to 500 cases. Evaluate each
    # complete 94-case repetition, retain all eleven, and require every gate.
    comparisons = [{"runIndex": index, "warmup": index == 0, "caseCount": len(selected["references"]),
                    **compare.compare(selected["references"], selected["before"], selected["after"],
                                      plan["semanticReviews"] if f16 else None)}
                   for index, selected in enumerate(rounds)]
    common.check(all(row["caseCount"] == 94 for row in comparisons), "incomplete quality repetition")
    quality = {"schemaVersion": 1, "rounds": comparisons, "rowCount": 1034, "positiveWerDeltaAllowed": 0,
               "automaticGatePassed": all(row["automaticGatePassed"] for row in comparisons),
               "semanticReviewComplete": all(row["semanticReviewComplete"] for row in comparisons)}
    common.write_private(plan["output"] / "quality-readback.json", quality)
    summary = {"schemaVersion": 1, "completed": True, "caseCount": 94, "rowCount": 1034,
               "qualityPassed": quality["automaticGatePassed"], "semanticReviewComplete": quality["semanticReviewComplete"],
               "cases": measurements, "L": None, "notCentralAggregateL": True, "qualified": False}
    common.write_private(plan["output"] / "measurement-readback.json", summary)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("central", type=Path)
    parser.add_argument("quality", type=Path)
    parser.add_argument("run_directory", type=Path)
    parser.add_argument("--encoder-precision", choices=("fp32", "fp16"), default="fp32")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        folder = args.run_directory.resolve()
        folder.relative_to(root / "artifacts")
        folder.mkdir(mode=0o700)
        freeze_path = folder / "freeze.json"
        prepare(root, args.central.resolve(), args.quality.resolve(), freeze_path, folder / "cases", args.encoder_precision)
        plan = preflight(root, freeze_path)
    except (OSError, ValueError, KeyError, TypeError, StopIteration, wave.Error, EOFError):
        print("MLX remainder preflight failed; no probe launched", flush=True)
        return 2
    handlers = {sig: signal.signal(sig, common.request_stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    cafe = subprocess.Popen(["/usr/bin/caffeinate", "-i", "-w", str(os.getpid())], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        report = common.execute(plan, lambda case, paths: launch_case(plan, case, paths))
        if not report["completed"]:
            return 1
        summary = readback(plan, report)
        print("MLX remainder complete; all 94 cases/1034 rows retained; quality=%s; semantic=%s" %
              (summary["qualityPassed"], summary["semanticReviewComplete"]), flush=True)
        return 0 if summary["qualityPassed"] and summary["semanticReviewComplete"] else 1
    finally:
        cafe.terminate()
        cafe.wait(timeout=5)
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


if __name__ == "__main__":
    raise SystemExit(main())
