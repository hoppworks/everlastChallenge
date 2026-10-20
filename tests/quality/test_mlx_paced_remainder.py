import importlib.util
import json
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

runner = load("mlx_remainder", ROOT / "scripts/run-mlx-paced-remainder.py")
fixtures = load("remainder_test_fixtures", Path(__file__).with_name("test_paced_remainder.py"))

class MlxRemainderTests(unittest.TestCase):
    def test_command_selects_only_the_qualified_encoder_variant(self):
        plan = {"root": Path("/work"), "binary": Path("/frozen/paced_runtime_probe"),
                "model": {"path": "models/weights.safetensors"}, "parameterPath": Path("/frozen/parameters.json")}
        case = {"audio": {"path": "audio.wav"}, "language": "de", "expectedOutcome": "Empty"}
        command = runner.command_for(plan, case, {"results": Path("/new/result.json")})
        self.assertEqual(command, ["/usr/bin/time", "-l", "/frozen/paced_runtime_probe", "/work/models/weights.safetensors",
                                  "mlx-reference-encoder", "/work/audio.wav", "de", "/frozen/parameters.json",
                                  "/new/result.json", "10", "--expect-empty"])

    def test_hybrid_backend_and_inactive_experiments_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, case, paths = fixtures.ExecutorTests.reports(Path(directory))
            metadata = json.loads(paths["metadata"].read_text())
            metadata.update(backend="mlxrs-encoder+whisper.cpp-decoder", contextCleanup=None,
                            pretranscriptionCandidate=None, fullPrefixCandidate=None)
            paths["metadata"].write_text(json.dumps(metadata))
            self.assertEqual(runner.validate_case(plan, case, paths)["errors"], [])
            for key, value in (("backend", "mlxrs"), ("pretranscriptionCandidate", True),
                               ("contextCleanup", {"enabled": True}), ("fullPrefixCandidate", True)):
                with self.subTest(key=key):
                    paths["metadata"].write_text(json.dumps(dict(metadata, **{key: value})))
                    self.assertTrue(runner.validate_case(plan, case, paths)["errors"])

    def test_streaming_bindings_validate_actual_bytes_and_reject_changed_or_unbounded_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            file = root / "model"
            file.write_bytes(b"model bytes")
            record = runner.binding(file, root)
            fingerprints = {}
            self.assertEqual(runner.verify(root, record, fingerprints, 11, True), b"model bytes")
            self.assertIn(file, fingerprints)
            for changed in (dict(record, sha256="0" * 64), dict(record, bytes=12), dict(record, path="../model")):
                with self.subTest(changed=changed), self.assertRaises(ValueError):
                    runner.verify(root, changed, {}, 11, True)
            file.write_bytes(b"other bytes")
            with self.assertRaises(ValueError):
                runner.verify(root, record, {}, 11, True)
            link = root / "linked"
            link.symlink_to(file)
            with self.assertRaises(OSError):
                runner.verify(root, dict(record, path="linked"), {}, 11, True)

    def test_case_percentiles_keep_all_ten_measurements(self):
        value = runner.stats([10, 9, 8, 7, 6, 5, 4, 3, 2, 1])
        self.assertEqual(value, {"count": 10, "medianNanos": 5.5, "p95Nanos": 10, "minNanos": 1, "maxNanos": 10})

    def test_stop_after_reaped_child_keeps_controlled_completion(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = {"root": root, "binary": root / "probe", "model": {"path": "weights.safetensors"},
                    "parameterPath": root / "parameters.json", "sourceRevision": "a" * 64}
            case = {"id": "case", "audio": {"path": "audio.wav"}, "language": "de", "expectedOutcome": "Local"}
            paths = {"results": root / "results.json", "log": root / "command.log"}
            process = mock.Mock(pid=12345)
            process.wait.return_value = 0
            def stop_at_readback(*_):
                runner.common.request_stop(runner.signal.SIGTERM, None)
                return {"errors": []}
            try:
                with mock.patch.object(runner.subprocess, "Popen", return_value=process), \
                     mock.patch.object(runner, "validate_case", side_effect=stop_at_readback), \
                     mock.patch("builtins.print"):
                    try:
                        result = runner.launch_case(plan, case, paths)
                    except KeyboardInterrupt:
                        self.fail("post-wait cancellation escaped before the complete slot report")
                self.assertEqual(result["exitCode"], 0)
                self.assertTrue(result["terminationConfirmed"])
                self.assertTrue(runner.common._INTERRUPTED)
                self.assertIsNone(runner.common._ACTIVE_PROCESS)
            finally:
                runner.common._INTERRUPTED = False
                runner.common._ACTIVE_PROCESS = None

    def test_complete_readback_checks_all_1034_rows_with_the_unchanged_bounded_oracle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cases, references, baseline = [], [], {}
            for index in range(94):
                plan, case, generated = fixtures.ExecutorTests.reports(root)
                case.update(id="case-%03d" % index, **{"class": "clean", "split": "acceptance"})
                cases.append(case)
                references.append({"id": case["id"], "language": "de", "class": "clean", "split": "acceptance",
                                   "reference": "private hypothesis"})
                baseline[case["id"]] = {"id": case["id"], "status": "ok", "text": "private hypothesis"}
                metadata = json.loads(generated["metadata"].read_text())
                metadata.update(backend=runner.BACKEND, contextCleanup=None,
                                pretranscriptionCandidate=None, fullPrefixCandidate=None)
                generated["metadata"].write_text(json.dumps(metadata))
                paths = runner.common.paths_for(root, index, case)
                generated["results"].rename(paths["results"])
                generated["metadata"].rename(paths["metadata"])
            plan.update(output=root, cases=cases, references=references, baseline=baseline)
            summary = runner.readback(plan, {"completed": True, "cases": cases})
            self.assertTrue(summary["qualityPassed"])
            self.assertTrue(summary["semanticReviewComplete"])
            self.assertEqual(summary["rowCount"], 1034)
            self.assertEqual(len(summary["cases"]), 94)
            quality = json.loads((root / "quality-readback.json").read_text())
            self.assertEqual(len(quality["rounds"]), 11)
            self.assertTrue(all(row["caseCount"] == 94 for row in quality["rounds"]))
