import importlib.util
from pathlib import Path
import unittest
from unittest import mock
import json
import subprocess
import sys
import tempfile

SPEC = importlib.util.spec_from_file_location("quality", Path(__file__).resolve().parents[2] / "scripts" / "quality.py")
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)

class QualityTests(unittest.TestCase):
    @staticmethod
    def case(identifier="a", reference="one", **changes):
        value = {"id": identifier, "language": "de", "class": "clean", "split": "acceptance", "reference": reference}
        value.update(changes)
        return value

    @staticmethod
    def result(identifier="a", text="one", status="ok"):
        return {"id": identifier, "status": status, "text": text}

    def test_minimal_levenshtein_and_micro_aggregation_are_separate_by_split_language_class(self):
        cases = [self.case("short", "one"), self.case("long", "one two three four five six seven eight nine"), self.case("swap", "a b", split="calibration"), self.case("insert", "one", language="en"), self.case("noise", "a b c", **{"class": "challenging"})]
        results = [self.result("short", "wrong"), self.result("long", "one two three four five six seven eight nine"), self.result("swap", "b a"), self.result("insert", "one two three"), self.result("noise", "a c")]
        report = quality.evaluate(cases, results)
        per_case = {item["id"]: item for item in report["cases"]}
        self.assertEqual(per_case["swap"]["edits"], 2)
        self.assertEqual(per_case["insert"]["wer"], 2.0)
        groups = {(item["split"], item["language"], item["class"]): item for item in report["groups"]}
        clean = groups[("acceptance", "de", "clean")]
        self.assertEqual((clean["edits"], clean["refWords"]), (1, 10))
        self.assertAlmostEqual(clean["wer"], 0.1)
        self.assertAlmostEqual(groups[("acceptance", "de", "challenging")]["wer"], 1 / 3)
        self.assertEqual(len(groups), 4)

    def test_empty_references_have_no_wer_and_flag_all_unwanted_non_whitespace_output(self):
        cases = [self.case("quiet", ""), self.case("speech", ""), self.case("punctuation", ""), self.case("normal", "one")]
        results = [self.result("quiet", "\u2003\n"), self.result("speech", "thank you"), self.result("punctuation", "..."), self.result("normal", "one")]
        report = quality.evaluate(cases, results)
        self.assertEqual([item["unwantedText"] for item in report["cases"]], [False, True, True, False])
        self.assertEqual([item["wer"] for item in report["cases"]], [None, None, None, 0.0])
        group = report["groups"][0]
        self.assertEqual((group["silenceCount"], group["unwantedTextCount"], group["refWords"]), (3, 2, 1))
        self.assertEqual(group["wer"], 0.0)

    def test_failed_and_missing_results_remain_visible_and_block_complete_group_wer(self):
        cases = [self.case("ok", "one"), self.case("failed", "one two"), self.case("missing", "one two three"), self.case("silence", "")]
        results = [self.result("ok", "one"), self.result("failed", None, "failed"), self.result("silence", None, "failed")]
        report = quality.evaluate(cases, results)
        self.assertEqual([item["status"] for item in report["cases"]], ["ok", "failed", "missing", "failed"])
        group = report["groups"][0]
        self.assertFalse(group["complete"])
        self.assertIsNone(group["wer"])
        self.assertEqual(group["observedWer"], 0.0)
        self.assertEqual((group["failedCount"], group["missingCount"], group["silenceFailedCount"], group["expectedRefWords"], group["refWords"]), (3, 1, 1, 6, 1))

    def test_duplicate_extra_and_malformed_records_fail_explicitly(self):
        invalid = [([self.case(), self.case()], []), ([self.case()], [self.result(), self.result()]), ([self.case()], [self.result("extra")]), ([self.case(language="fr")], []), ([self.case(**{"class": "other"})], []), ([self.case(split="unknown")], []), ([self.case()], [self.result(text=None)]), ([self.case()], [self.result(status="unknown")])]
        for cases, results in invalid:
            with self.subTest(cases=cases, results=results), self.assertRaises(quality.QualityError):
                quality.evaluate(cases, results)
        # Documented corpus metadata is not part of WER.
        self.assertEqual(quality.evaluate([self.case(audio="local.wav", tags=["anchor"], source={"license": "fixture"})], [self.result()])["cases"][0]["edits"], 0)

    def test_bounds_and_fixed_unicode_are_checked_before_distance_work(self):
        invalid = [([], []), ([self.case(str(i)) for i in range(501)], []), ([self.case(reference="é" * (quality.MAX_TEXT_BYTES // 2 + 1))], []), ([self.case(reference="a " * (quality.MAX_TOKENS + 1))], []), ([self.case(reference="a " * 1500)], [self.result(text="b " * 1500)]), ([self.case()], [self.result(text="\ud800")])]
        for cases, results in invalid:
            with self.assertRaises(quality.QualityError):
                quality.evaluate(cases, results)
        with mock.patch.object(quality.unicodedata, "unidata_version", "14.0.0"):
            with self.assertRaises(quality.QualityError):
                quality.evaluate([self.case()], [self.result()])

    def test_cli_writes_explicit_numeric_report_and_rejects_wrong_manifest_unicode_without_text_leak(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            results = Path(directory) / "results.json"
            report = Path(directory) / "report.json"
            payload = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": [self.case(reference="secret reference words")]}
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            results.write_text(json.dumps([self.result(text="secret reference words")]), encoding="utf-8")
            command = [sys.executable, str(Path(quality.__file__)), str(manifest), str(results), "--output", str(report)]
            process = subprocess.run(command, capture_output=True, text=True, timeout=5)
            self.assertEqual(process.returncode, 0, process.stderr)
            parsed = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(parsed["cases"][0]["edits"], 0)
            self.assertNotIn("secret reference words", report.read_text(encoding="utf-8") + process.stdout + process.stderr)
            before = report.read_bytes()
            payload["unicodeVersion"] = "14.0.0"
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            process = subprocess.run(command, capture_output=True, text=True, timeout=5)
            self.assertNotEqual(process.returncode, 0)
            self.assertEqual(report.read_bytes(), before)
            self.assertNotIn("secret reference words", process.stdout + process.stderr)

    def test_all_unicode_white_space_and_only_that_property_splits_tokens(self):
        whitespace = "\t\n\v\f\r \u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000"
        for char in whitespace:
            self.assertEqual(quality.normalize("a" + char + "b"), ["a", "b"])
        for char in "\u001c\u001d\u001e\u001f\u200b\ufeff":
            self.assertEqual(quality.normalize("a" + char + "b"), ["a" + char + "b"])
        self.assertEqual(quality.normalize("İ"), ["i\u0307"])
        self.assertEqual(quality.normalize("Straße STRASSE"), ["straße", "strasse"])

    def test_no_number_stopword_or_hallucination_filter_can_reduce_word_distance(self):
        examples = [("zwölf", "12", 1), ("nicht 12", "12", 1), ("und", "", 1), ("a b c", "a b c danke", 1), ("a b c", "", 3)]
        for reference, hypothesis, edits in examples:
            report = quality.evaluate([self.case(reference=reference)], [self.result(text=hypothesis)])
            self.assertEqual(report["cases"][0]["edits"], edits)

    def test_total_work_budget_and_exact_text_token_limits(self):
        self.assertEqual(len(quality.normalize("x" * quality.MAX_TEXT_BYTES)), 1)
        self.assertEqual(len(quality.normalize("a " * quality.MAX_TOKENS)), quality.MAX_TOKENS)
        cases = [self.case(str(index), "a " * 1400) for index in range(11)]
        results = [self.result(str(index), "b " * 1400) for index in range(11)]
        with mock.patch.object(quality, "_distance", side_effect=AssertionError("distance must not run")):
            with self.assertRaises(quality.QualityError):
                quality.evaluate(cases, results)

    def test_cli_bounded_json_and_duplicate_keys_fail_without_overwriting_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            results = Path(directory) / "results.json"
            report = Path(directory) / "report.json"
            manifest.write_text(json.dumps({"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": [self.case()]}), encoding="utf-8")
            command = [sys.executable, str(Path(quality.__file__)), str(manifest), str(results), "--output", str(report)]
            for invalid in [b" " * (quality.MAX_FILE_BYTES + 1), b'[{"id":"a","id":"b","status":"ok","text":"private content"}]', b'[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[', b'NaN']:
                results.write_bytes(invalid)
                process = subprocess.run(command, capture_output=True, text=True, timeout=5)
                self.assertNotEqual(process.returncode, 0)
                self.assertFalse(report.exists())
                self.assertNotIn("private content", process.stdout + process.stderr)
            results.write_text(json.dumps([self.result()]), encoding="utf-8")
            before = manifest.read_bytes()
            command[-1] = str(manifest)
            process = subprocess.run(command, capture_output=True, text=True, timeout=5)
            self.assertNotEqual(process.returncode, 0)
            self.assertEqual(manifest.read_bytes(), before)

    def test_result_order_does_not_change_scores_or_manifest_order(self):
        cases = [self.case("first", "a"), self.case("second", "b")]
        results = [self.result("first", "b"), self.result("second", "b")]
        self.assertEqual(quality.evaluate(cases, results), quality.evaluate(cases, list(reversed(results))))
        self.assertEqual([case["id"] for case in quality.evaluate(cases, results)["cases"]], ["first", "second"])

    def test_cli_never_overwrites_an_existing_report(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "manifest.json"
            results = Path(directory) / "results.json"
            report = Path(directory) / "report.json"
            manifest.write_text(json.dumps({"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": [self.case()]}), encoding="utf-8")
            results.write_text(json.dumps([self.result()]), encoding="utf-8")
            report.write_bytes(b"existing frozen measurement")
            process = subprocess.run([sys.executable, str(Path(quality.__file__)), str(manifest), str(results), "--output", str(report)], capture_output=True, text=True, timeout=5)
            self.assertNotEqual(process.returncode, 0)
            self.assertEqual(report.read_bytes(), b"existing frozen measurement")
            self.assertEqual(list(Path(directory).glob(".quality-*")), [])

    def test_normalization_is_nfc_lower_punctuation_space_without_digit_or_symbol_mapping(self):
        self.assertEqual(quality.normalize("CAFE\u0301—12 € A+B ① 1,5"), ["café", "12", "€", "a+b", "①", "1", "5"])
        self.assertEqual(quality.normalize("a\u001cb\u00a0c\u2003d\u0085e"), ["a\u001cb", "c", "d", "e"])

if __name__ == "__main__":
    unittest.main()
