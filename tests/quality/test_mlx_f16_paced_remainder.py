"""Precision evidence at the existing paced remainder validation boundary."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load("mlx_f16_remainder", ROOT / "scripts/run-mlx-paced-remainder.py")
fixtures = load("mlx_f16_remainder_fixtures", Path(__file__).with_name("test_paced_remainder.py"))


class F16RemainderTests(unittest.TestCase):
    def test_f16_watchdog_preserves_its_cause_and_uses_the_same_conservative_reaper(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            process = runner.subprocess.Popen(["/usr/bin/python3", "-c", "import time; time.sleep(30)"], start_new_session=True)
            actual_wait, actual_killpg = process.wait, os.killpg
            waits, observations = 0, 0
            def wait(timeout=None):
                nonlocal waits
                waits += 1
                if waits == 1:
                    raise runner.subprocess.TimeoutExpired("owned probe", timeout)
                return actual_wait(timeout=timeout)
            def signal_group(group, signum):
                nonlocal observations
                self.assertEqual(group, process.pid)
                if signum == 0:
                    observations += 1
                    if observations == 1:
                        raise PermissionError(1, "Operation not permitted")
                return actual_killpg(group, signum)
            plan = {"root": root, "binary": root / "unused", "model": {"path": "unused-model"},
                    "parameterPath": root / "unused-parameters", "sourceRevision": "a" * 64, "encoderPrecision": "fp16"}
            case = {"id": "case", "audio": {"path": "unused.wav"}, "language": "de", "expectedOutcome": "Local"}
            paths = {"results": root / "result.json", "metadata": root / "metadata.json", "log": root / "run.log"}
            try:
                with mock.patch.object(runner.subprocess, "Popen", return_value=process), \
                     mock.patch.object(process, "wait", side_effect=wait), \
                     mock.patch.object(runner.os, "killpg", side_effect=signal_group), mock.patch("builtins.print"):
                    result = runner.launch_case(plan, case, paths)
                self.assertTrue(result["watchdogExpired"])
                self.assertIn("watchdog expired", result["launchError"])
                self.assertTrue(result["terminationConfirmed"])
                self.assertFalse(result["forcedGroupCleanup"])
                self.assertEqual(result["exitCode"], -15)
                self.assertEqual(result["cleanupObservationErrors"][0]["errno"], 1)
            finally:
                if process.poll() is None:
                    process.kill()
                    actual_wait(timeout=5)

    def test_f16_prepare_cannot_bypass_failed_or_unreviewed_central_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            central, quality = root / "central", root / "quality"
            central.mkdir()
            quality.mkdir()
            (central / "freeze.json").write_text("{}")
            for completion in ({"completed": False}, {
                "completed": True, "semanticReviewComplete": True, "candidateAbsoluteLimitsPassed": True,
                "allCaseMediansImproved": True, "allCaseP95Improved": True, "powerGatePassed": True,
                "bothPairOrdersComplete": True, "kind": "unbound-assertion",
            }):
                (central / "completion.json").write_text(json.dumps(completion))
                with self.assertRaises(ValueError):
                    runner.prepare(root, central, quality, root / "freeze.json", root / "cases", encoder_precision="fp16")
                self.assertFalse((root / "freeze.json").exists())

    def test_f16_launch_does_not_admit_the_next_case_when_group_absence_is_denied(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = root / "probe"
            probe.write_text("#!/usr/bin/python3\npass\n")
            probe.chmod(0o700)
            plan = {"root": root, "binary": probe, "model": {"path": "unused-model"},
                    "parameterPath": root / "unused-parameters", "sourceRevision": "a" * 64,
                    "encoderPrecision": "fp16"}
            case = {"id": "case", "audio": {"path": "unused.wav"}, "language": "de", "expectedOutcome": "Local"}
            paths = {"results": root / "result.json", "metadata": root / "metadata.json", "log": root / "run.log"}
            with mock.patch.object(runner.os, "killpg", side_effect=PermissionError(1, "Operation not permitted")), \
                 mock.patch("builtins.print"):
                result = runner.launch_case(plan, case, paths)
            self.assertFalse(result["terminationConfirmed"], "wait alone cannot certify that the owned group is absent")
            self.assertTrue(result["launchError"])
            self.assertIsNone(runner.common._ACTIVE_PROCESS)

    def test_quality_admission_preserves_both_sets_and_exact_full_object_reviews(self):
        compare = load("f16_remainder_admission_oracle", ROOT / "scripts/compare_mlx_quality.py")
        cases = [{"id": "case-%03d" % i, "language": "de", "class": "clean", "split": "acceptance",
                  "reference": "private hypothesis", "source": {"recording": i}} for i in range(100)]
        cases[95].update(language="en", reference="And an experiment.")
        baseline = [{"id": case["id"], "status": "ok", "text": case["reference"]} for case in cases]
        baseline[95]["text"] = "an experiment."
        candidate = [{"id": case["id"], "status": "ok", "text": case["reference"]} for case in cases]
        approved = compare.compare([cases[95]], [baseline[95]], [candidate[95]])["reviewsRequired"][0]
        reviews = {cases[95]["id"]: {"pairSha256": approved["pairSha256"], "accepted": True,
                                     "note": "Full public case and conjunction restoration reviewed."}}
        manifest = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": cases}
        canonical = {**manifest, "cases": cases[:90]}
        admitted = runner.reviewed_quality(manifest, canonical, baseline, candidate, reviews)
        self.assertEqual(len(admitted), 100)
        self.assertEqual(admitted[cases[95]["id"]], cases[95])
        candidate[95]["text"] = "Not an experiment."
        with self.assertRaises(ValueError, msg="equal WER cannot authorize a new unreviewed negation"):
            runner.reviewed_quality(manifest, canonical, baseline, candidate, reviews)

    def test_f16_replay_requires_the_actual_loaded_compute_precision(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            plan, case, paths = fixtures.ExecutorTests.reports(folder)
            plan["encoderPrecision"] = "fp16"
            plan["parameters"]["runtime"] = {
                "encoderDtype": "fp16", "encoderCheckpointDtype": "fp32",
                "encoderFeatureDtype": "fp32",
            }
            plan["model"] = {"id": runner.MODEL_ID, "sha256": runner.MODEL_SHA}
            metadata = json.loads(paths["metadata"].read_text())
            metadata.update(parameters=plan["parameters"], model=plan["model"],
                            backend=runner.BACKEND, contextCleanup=None,
                            pretranscriptionCandidate=None, fullPrefixCandidate=None)
            paths["metadata"].write_text(json.dumps(metadata))
            paths["log"] = folder / "run.log"
            loaded = {
                "kind": "mlx-reference-encoder-loaded", "encoderComputeDtype": "fp32",
                "checkpointDtype": "fp32", "bridgeFeatureDtype": "fp32",
                "checkpointSha256": runner.MODEL_SHA,
                "decoderModelSha256": "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
            }
            paths["log"].write_text(json.dumps(loaded) + "\n")
            self.assertTrue(runner.validate_case(plan, case, paths)["errors"],
                            "a configured F16 label cannot certify an actually loaded F32 encoder")
            loaded["encoderComputeDtype"] = "fp16"
            paths["log"].write_text(json.dumps(loaded) + "\n")
            self.assertEqual(runner.validate_case(plan, case, paths)["errors"], [])
            paths["log"].unlink()
            self.assertTrue(runner.validate_case(plan, case, paths)["errors"],
                            "missing actual precision evidence must block admission")

    def test_all_eleven_rounds_keep_an_exactly_reviewed_improvement(self):
        compare = load("f16_remainder_original_oracle", ROOT / "scripts/compare_mlx_quality.py")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cases, references, baseline, quality_cases = [], [], {}, {}
            for index in range(94):
                plan, case, generated = fixtures.ExecutorTests.reports(root)
                case.update(id="case-%03d" % index, **{"class": "clean", "split": "acceptance"})
                reference = "private hypothesis"
                before = after = reference
                if index == 0:
                    case["language"] = "en"
                    reference = after = "And an experiment."
                    before = "an experiment."
                cases.append(case)
                full_case = {key: case[key] for key in ("id", "language", "class", "split", "audio")}
                full_case["reference"] = reference
                quality_cases[case["id"]] = full_case
                references.append({key: full_case[key] for key in ("id", "language", "class", "split", "reference")})
                baseline[case["id"]] = {"id": case["id"], "status": "ok", "text": before}
                plan["parameters"]["runtime"] = {"encoderDtype": "fp16", "encoderCheckpointDtype": "fp32",
                                                   "encoderFeatureDtype": "fp32"}
                plan["model"] = {"id": runner.MODEL_ID, "sha256": runner.MODEL_SHA}
                rows = json.loads(generated["results"].read_text())
                for row in rows:
                    row["text"] = after
                generated["results"].write_text(json.dumps(rows))
                metadata = json.loads(generated["metadata"].read_text())
                metadata.update(parameters=plan["parameters"], model=plan["model"], language=case["language"],
                                backend=runner.BACKEND, contextCleanup=None,
                                pretranscriptionCandidate=None, fullPrefixCandidate=None)
                generated["metadata"].write_text(json.dumps(metadata))
                paths = runner.common.paths_for(root, index, case)
                generated["results"].rename(paths["results"])
                generated["metadata"].rename(paths["metadata"])
                paths["log"].write_text(json.dumps({
                    "kind": "mlx-reference-encoder-loaded", "encoderComputeDtype": "fp16",
                    "checkpointDtype": "fp32", "bridgeFeatureDtype": "fp32",
                    "checkpointSha256": runner.MODEL_SHA, "decoderModelSha256": runner.DECODER_MODEL_SHA,
                }) + "\n")
            identifier = cases[0]["id"]
            after = {"id": identifier, "status": "ok", "text": "And an experiment."}
            approval = compare.compare([quality_cases[identifier]], [baseline[identifier]], [after])["reviewsRequired"][0]
            reviews = {identifier: {"pairSha256": approval["pairSha256"], "accepted": True,
                                     "note": "Restores the published conjunction; complete object reviewed."}}
            plan.update(output=root, cases=cases, references=references, baseline=baseline,
                        qualityCases=quality_cases, semanticReviews=reviews, encoderPrecision="fp16")
            summary = runner.readback(plan, {"completed": True, "cases": cases})
            self.assertTrue(summary["qualityPassed"])
            self.assertTrue(summary["semanticReviewComplete"], "slot IDs must not invalidate the approved full case/text/status pair")
            quality = json.loads((root / "quality-readback.json").read_text())
            self.assertEqual(len(quality["rounds"]), 11)
            self.assertTrue(all(row["caseCount"] == 94 and row["qualified"] for row in quality["rounds"]))
