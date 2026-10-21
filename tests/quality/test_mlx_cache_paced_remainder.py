"""Admit the measured cache policy through the existing replay boundary."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load("mlx_cache_remainder", ROOT / "scripts/run-mlx-paced-remainder.py")
fixtures = load("mlx_cache_remainder_fixtures", Path(__file__).with_name("test_paced_remainder.py"))


class CacheRemainderTests(unittest.TestCase):
    def test_selected_cache_profile_requires_its_actual_budget_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            plan, case, paths = fixtures.ExecutorTests.reports(folder)
            plan.update(encoderPrecision="fp16", cacheBudgetBytes=268435456,
                        model={"id": runner.MODEL_ID, "sha256": runner.MODEL_SHA})
            plan["parameters"]["runtime"] = {
                "encoderDtype": "fp16", "encoderCheckpointDtype": "fp32",
                "encoderFeatureDtype": "fp32",
            }
            metadata = json.loads(paths["metadata"].read_text())
            metadata.update(parameters=plan["parameters"], model=plan["model"],
                            backend=runner.BACKEND, contextCleanup=None,
                            pretranscriptionCandidate=None, fullPrefixCandidate=None)
            paths["metadata"].write_text(json.dumps(metadata))
            paths["log"] = folder / "run.log"
            loaded = {
                "kind": "mlx-reference-encoder-loaded", "encoderComputeDtype": "fp16",
                "checkpointDtype": "fp32", "bridgeFeatureDtype": "fp32",
                "checkpointSha256": runner.MODEL_SHA,
                "decoderModelSha256": runner.DECODER_MODEL_SHA,
            }
            paths["log"].write_text(json.dumps(loaded) + "\n")
            self.assertTrue(runner.validate_case(plan, case, paths)["errors"],
                            "FP16 alone cannot certify the selected RAM policy")
            installed = {"kind": "mlx-cache-budget-installed", "bytes": 268435456}
            paths["log"].write_text(json.dumps(installed) + "\n" + json.dumps(loaded) + "\n")
            self.assertEqual(runner.validate_case(plan, case, paths)["errors"], [])
