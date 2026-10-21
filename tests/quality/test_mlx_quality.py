import importlib.util
from pathlib import Path
import unittest


class MlxQualityTests(unittest.TestCase):
    @staticmethod
    def module():
        path = Path(__file__).resolve().parents[2] / "scripts/compare_mlx_quality.py"
        spec = importlib.util.spec_from_file_location("mlx_quality", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_even_a_tenth_percentage_point_regression_fails_without_inherited_tolerance(self):
        compare = self.module().compare
        cases = [{"id": "one", "reference": " ".join(["word"] * 1000),
                  "language": "de", "class": "clean", "split": "acceptance"}]
        baseline = [{"id": "one", "status": "ok", "text": " ".join(["word"] * 999)}]
        candidate = [{"id": "one", "status": "ok", "text": " ".join(["word"] * 998)}]
        self.assertFalse(compare(cases, baseline, candidate)["automaticGatePassed"])

    def test_changed_text_requires_review_bound_to_the_exact_pair(self):
        compare = self.module().compare
        cases = [{"id": "one", "reference": "Hallo Welt", "language": "de",
                  "class": "clean", "split": "acceptance"}]
        baseline = [{"id": "one", "status": "ok", "text": "Hallo Welt"}]
        candidate = [{"id": "one", "status": "ok", "text": "Hallo Welt!"}]
        report = compare(cases, baseline, candidate)
        self.assertTrue(report["automaticGatePassed"])
        self.assertFalse(report["qualified"])
        pair = report["reviewsRequired"][0]
        stale = {"one": {"pairSha256": "0" * 64, "accepted": True,
                         "note": "punctuation checked"}}
        self.assertFalse(compare(cases, baseline, candidate, stale)["qualified"])
        reviewed = {"one": {"pairSha256": pair["pairSha256"], "accepted": True,
                            "note": "punctuation checked"}}
        self.assertTrue(compare(cases, baseline, candidate, reviewed)["qualified"])

