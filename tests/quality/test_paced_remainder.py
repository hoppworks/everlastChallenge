import importlib.util
import json
from pathlib import Path
import tempfile
import os
import signal
import subprocess
import sys
import time
import io
import contextlib
import wave
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("paced_remainder", Path(__file__).resolve().parents[2] / "scripts" / "run-paced-remainder.py")
executor = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(executor)


class ExecutorTests(unittest.TestCase):
    def test_first_launch_failure_preserves_all_94_cases_without_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            plan = {"output": Path(directory) / "new", "cases": [{"id": "case-%03d" % i} for i in range(94)]}
            calls = []

            def launch(case, paths):
                calls.append(case["id"])
                return {"exitCode": None, "launchError": "controlled launch failure", "wrapperPid": None}

            report = executor.execute(plan, launch)
            self.assertEqual(calls, ["case-000"])
            self.assertEqual(len(report["cases"]), 94)
            self.assertEqual(report["cases"][0]["status"], "failed")
            self.assertEqual([c["status"] for c in report["cases"]][1:], ["unavailable"] * 93)
            self.assertFalse(report["completed"])
            self.assertIsNone(report["statistics"])
            self.assertEqual(sum(len(c["rows"]) for c in report["cases"]), 1034)

    def test_success_requires_all_eleven_rows_and_preserves_empty_without_L(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, case, paths = self.reports(Path(directory))
            validated = executor.validate_case(plan, case, paths)
            self.assertEqual(validated["errors"], [])
            self.assertEqual(len(validated["rows"]), 11)
            self.assertEqual(validated["rows"][-1]["id"], "case::measured-010")
            metadata = json.loads(paths["metadata"].read_text())
            metadata["runs"][5]["feed"]["publications"][0]["beforePublishNanos"] = 19_999_999
            paths["metadata"].write_text(json.dumps(metadata))
            self.assertTrue(executor.validate_case(plan, case, paths)["errors"])

    @staticmethod
    def reports(directory, empty=False):
        case = {"id": "case", "language": "de", "expectedOutcome": "Empty" if empty else "Local", "audio": {"frames": 320, "sha256": "c" * 64}}
        plan = {"parameters": {"test": True}, "parameterSha256": "d" * 64, "sourceRevision": "a" * 40,
                "model": {"id": "large-v3-turbo-q5_0", "sha256": "b" * 64}, "publisherPolicy": "timeConstrained"}
        paths = {"results": directory / "results.json", "metadata": directory / "metadata.json"}
        results, rows = [], []
        for index in range(11):
            identifier = "warmup" if index == 0 else "measured-%03d" % index
            results.append({"id": identifier, "warmup": index == 0, "status": "ok", "text": "" if empty else "private hypothesis"})
            rows.append({"id": identifier, "warmup": index == 0, "status": "ok", "session": index + 1,
                "expectedOutcome": case["expectedOutcome"], "error": None, "controllerClockMatchesCapture": True,
                "engineInputFrames": 320, "engineElapsedNanos": 10_000_000, "engineRtf": .5, "engineRtfUnavailableReason": None,
                "tStopOffsetNanos": 20_000_100, "tTerminalOffsetNanos": 31_000_100, "stopToTerminalNanos": 11_000_000,
                "tTextOffsetNanos": None if empty else 31_000_100, "stopToTextNanos": None if empty else 11_000_000,
                "terminal": case["expectedOutcome"], "referenceMatch": None,
                "qosError": None, "qosEntered": False, "qosFinished": False, "publisherJoined": True,
                "feed": {"timingValid": True, "plannedStopOffset": {"secs": 0, "nanos": 20_000_000},
                    "waitSummary": {"observedPublications": 1, "detailedPublications": 0},
                    "publications": [{"startFrame": 0, "endFrame": 320, "plannedNanos": 20_000_000,
                        "beforePublishNanos": 20_000_001, "afterPublishNanos": 20_000_010, "deviationNanos": 10, "timing": "onBudget"}]}})
        requested = {"periodTicks": 480000, "computationTicks": 12000, "constraintTicks": 24000, "preemptible": True}
        snapshot = {"timeConstraint": requested, "defaultPolicy": False, "basicPolicy": 2, "cpu": {"userNanos": 1000, "systemNanos": 2000}}
        for row in rows:
            row["publisher"] = {"policy": "timeConstrained", "requested": requested,
                "before": dict(snapshot, defaultPolicy=True, basicPolicy=1), "configured": snapshot, "after": snapshot,
                "beforeFeed": {"snapshot": snapshot, "error": None}, "afterFeed": {"snapshot": snapshot, "error": None},
                "outliers": [], "setupError": None, "postError": None, "finishError": None}
        metadata = {"schemaVersion": 1, "kind": "paced-runtime-complete-recording", "passed": True,
                    "debugAssertions": False, "sourceRevision": plan["sourceRevision"], "fixtureSha256": case["audio"]["sha256"],
                    "inputFrames": 320, "sampleRate": 16000, "channels": 1, "model": plan["model"], "language": "de",
                    "parameterDocumentSha256": plan["parameterSha256"], "parameters": plan["parameters"],
                    "referenceSha256": None, "warmupRuns": 1, "measuredRunsRequested": 10, "loadError": None,
                    "expectedOutcome": case["expectedOutcome"], "cleanup": {"joined": True, "error": None},
                    "runs": rows, "engineTimingCoverage": {"measuredRuns": 10, "withActualEngineTiming": 10},
                    "processId": 123, "qualified": False, "L": None}
        for path, value in [(paths["results"], results), (paths["metadata"], metadata)]:
            path.write_text(json.dumps(value))
            path.chmod(0o600)
        return plan, case, paths

    def test_sparse_oversize_metadata_is_rejected_before_reading(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "large.json"
            with path.open("wb") as stream:
                stream.truncate(executor.MAX_METADATA + 1)
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("oversize contents read")):
                with self.assertRaises(ValueError):
                    executor.read_json(path, executor.MAX_METADATA)

    def test_bindings_use_actual_bytes_mode_and_reject_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source.bin"
            source.write_bytes(b"source")
            source.chmod(0o600)
            binding = {"path": "source.bin", "bytes": 6, "sha256": executor.sha256(b"source"), "mode": "0o600"}
            fingerprints = {}
            self.assertEqual(executor.verify_binding(root, binding, fingerprints, 10, True), b"source")
            self.assertIn(source, fingerprints)
            for key, value in [("bytes", 5), ("sha256", "0" * 64), ("mode", "0o644"), ("path", "../source.bin")]:
                with self.subTest(key=key), self.assertRaises(ValueError):
                    executor.verify_binding(root, dict(binding, **{key: value}), {}, 10, True)
            link = root / "link.bin"
            link.symlink_to(source)
            with self.assertRaises(OSError):
                executor.verify_binding(root, dict(binding, path="link.bin"), {}, 10, True)
            with self.assertRaises(ValueError):
                executor.parse_json(b'{"id":1,"id":2}')
            with self.assertRaises(ValueError):
                executor.parse_json(b'{"number":NaN}')

    def test_compiled_source_pins_are_deduplicated_without_conflict(self):
        pin = {"filename": "source.rs", "sha256": "a" * 64}
        self.assertEqual(executor.compiled_sources({"sourceHashes": {"one": pin, "nested": [pin]}}), {"source.rs": "a" * 64})
        with self.assertRaises(ValueError):
            executor.compiled_sources([pin, dict(pin, sha256="b" * 64)])

    def test_TC_readback_demotion_is_not_accepted_as_status_ok(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, case, paths = self.reports(Path(directory))
            metadata = json.loads(paths["metadata"].read_text())
            metadata["runs"][3]["publisher"]["after"]["defaultPolicy"] = True
            paths["metadata"].write_text(json.dumps(metadata))
            self.assertTrue(executor.validate_case(plan, case, paths)["errors"])

    def test_command_is_fixed_time_wrapper_and_Local_has_no_exact_reference_argument(self):
        plan = {"root": Path("/work"), "binary": Path("/work/target/release/examples/paced_runtime_probe"),
                "model": {"path": "artifacts/model.bin"}, "parameterPath": Path("/work/params.json")}
        case = {"audio": {"path": "audio.wav"}, "language": "en", "expectedOutcome": "Local"}
        paths = {"results": Path("/work/new/results.json")}
        expected = ["/usr/bin/time", "-l", "/work/target/release/examples/paced_runtime_probe", "/work/artifacts/model.bin",
                    "reference", "/work/audio.wav", "en", "/work/params.json", "/work/new/results.json", "10"]
        self.assertEqual(executor.command_for(plan, case, paths), expected)
        case["expectedOutcome"] = "Empty"
        self.assertEqual(executor.command_for(plan, case, paths), expected + ["--expect-empty"])

    def test_watchdog_kills_only_the_owned_group_and_waits_for_terminal_wrapper(self):
        with tempfile.TemporaryDirectory() as directory:
            process = mock.Mock(pid=12345)
            process.wait.side_effect = [executor.subprocess.TimeoutExpired(["fixed"], 7200), -9]
            plan = {"root": Path("/work"), "binary": Path("/work/target/release/examples/paced_runtime_probe"),
                    "model": {"path": "model.bin"}, "parameterPath": Path("/work/p.json"), "sourceRevision": "a" * 40}
            case = {"id": "case-known", "audio": {"path": "audio.wav"}, "language": "de", "expectedOutcome": "Local"}
            paths = {"results": Path(directory) / "new.json", "log": Path(directory) / "log"}
            killed = []
            def kill_group(pid, sig):
                if sig == 0:
                    raise ProcessLookupError()
                killed.append((pid, sig))
            with mock.patch.object(executor.subprocess, "Popen", return_value=process) as start, mock.patch.object(executor.os, "killpg", side_effect=kill_group):
                progress = io.StringIO()
                with contextlib.redirect_stdout(progress):
                    result = executor.launch_case(plan, case, paths)
            progress_row = json.loads(progress.getvalue())
            self.assertEqual(set(progress_row), {"caseId", "wrapperPid", "startedUtc"})
            self.assertEqual(progress_row["caseId"], "case-known")
            self.assertEqual(progress_row["wrapperPid"], 12345)
            self.assertNotIn("model.bin", progress.getvalue())
            self.assertTrue(result.get("watchdogExpired"))
            self.assertTrue(result["terminationConfirmed"])
            self.assertEqual(killed, [(12345, executor.signal.SIGKILL)])
            self.assertEqual(result["wrapperPid"], 12345)
            self.assertEqual(process.wait.call_args_list, [mock.call(timeout=7200), mock.call(timeout=10)])
            self.assertTrue(start.call_args.kwargs["start_new_session"])
            self.assertEqual(paths["log"].stat().st_mode & 0o777, 0o600)

    def test_malformed_result_scalar_keeps_global_slots_instead_of_crashing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan, case, _ = self.reports(root)
            plan.update(output=root / "new", cases=[dict(case, id="case-%03d" % i) for i in range(94)])
            def launch(_case, paths):
                _, _, generated = self.reports(paths["results"].parent)
                results = json.loads(generated["results"].read_text())
                results[0] = 1
                generated["results"].write_text(json.dumps(results))
                generated["results"].rename(paths["results"])
                generated["metadata"].rename(paths["metadata"])
                return {"exitCode": 0, "launchError": None, "wrapperPid": 999, "terminationConfirmed": True}
            report = executor.execute(plan, launch)
            self.assertEqual(len(report["cases"]), 94)
            self.assertEqual(report["cases"][0]["status"], "failed")
            self.assertTrue(all(c["status"] == "unavailable" for c in report["cases"][1:]))

    def test_all_94_successes_are_serial_and_never_export_hypothesis_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan, case, _ = self.reports(root)
            plan.update(output=root / "new", cases=[dict(case, id="case-%03d" % i) for i in range(94)])
            launched = []
            def launch(case, paths):
                self.assertEqual(len(launched), int(case["id"][-3:]))
                launched.append(case["id"])
                _, _, generated = self.reports(paths["results"].parent, empty=case["expectedOutcome"] == "Empty")
                generated["results"].rename(paths["results"])
                generated["metadata"].rename(paths["metadata"])
                paths["log"].write_bytes(b"resource observations only")
                paths["log"].chmod(0o600)
                return {"exitCode": 0, "launchError": None, "wrapperPid": 999, "terminationConfirmed": True}
            report = executor.execute(plan, launch)
            self.assertTrue(report["completed"])
            self.assertEqual(len(launched), 94)
            self.assertEqual(report["rowCount"], 1034)
            self.assertTrue(all(row["validated"] for case in report["cases"] for row in case["rows"]))
            self.assertNotIn("private hypothesis", json.dumps(report))
            self.assertIsNone(report["statistics"])
            first_completion = json.loads((plan["output"] / "001-case-000.json.completion.json").read_text())
            self.assertEqual(first_completion["launch"]["wrapperPid"], 999)
            self.assertEqual(first_completion["actualProbePid"], 123)
            with self.assertRaises(FileExistsError):
                executor.execute(plan, launch)
            self.assertEqual(len(launched), 94)

    def test_empty_outcome_and_malformed_missing_extra_duplicate_rows_are_not_silent(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, case, paths = self.reports(Path(directory), empty=True)
            self.assertEqual(executor.validate_case(plan, case, paths)["errors"], [])
            original_result = paths["results"].read_bytes()
            original_metadata = paths["metadata"].read_bytes()
            for mutation in ["missing", "extra", "duplicate", "scalar", "text", "cleanup", "coverage", "pid", "requested", "engine", "rawBefore"]:
                results = json.loads(original_result)
                metadata = json.loads(original_metadata)
                if mutation == "missing": results.pop()
                elif mutation == "extra": results.append(results[-1])
                elif mutation == "duplicate": results[1]["id"] = "warmup"
                elif mutation == "scalar": results[1] = None
                elif mutation == "text": results[1]["text"] = "unexpected output"
                elif mutation == "cleanup": metadata["cleanup"] = []
                elif mutation == "coverage": metadata["engineTimingCoverage"]["withActualEngineTiming"] = 9
                elif mutation == "pid": metadata["processId"] = None
                elif mutation == "requested": metadata["runs"][2]["publisher"]["requested"] = None
                elif mutation == "engine": metadata["runs"][2]["engineRtf"] = 0
                elif mutation == "rawBefore": metadata["runs"][2]["publisher"]["before"]["defaultPolicy"] = None
                paths["results"].write_text(json.dumps(results))
                paths["metadata"].write_text(json.dumps(metadata))
                with self.subTest(mutation=mutation):
                    self.assertTrue(executor.validate_case(plan, case, paths)["errors"])

    def test_private_publication_and_changed_preflight_identity_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            executor.write_private(path, {"retained": True})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                executor.write_private(path, {"retained": False})
            self.assertEqual(json.loads(path.read_text()), {"retained": True})
            plan = {"fingerprints": {path: executor.fingerprint(path.stat())}}
            executor.check_unchanged(plan)
            path.write_bytes(b"changed")
            with self.assertRaises(ValueError):
                executor.check_unchanged(plan)

    def test_real_owned_time_wrapper_group_ends_and_foreign_group_survives(self):
        with tempfile.TemporaryDirectory() as directory:
            ready = Path(directory) / "ready.json"
            helper = "import json,os,sys,time; open(sys.argv[1],'w').write(json.dumps({'pid':os.getpid(),'group':os.getpgrp()})); time.sleep(60)"
            owned = foreign = None
            try:
                owned = subprocess.Popen(["/usr/bin/time", "-l", sys.executable, "-c", helper, str(ready)],
                                         start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                foreign = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"],
                                           start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                deadline = time.monotonic() + 3
                while not ready.exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertTrue(ready.exists(), "owned helper did not become ready")
                child = json.loads(ready.read_text())
                self.assertEqual(child["group"], owned.pid)
                self.assertNotEqual(child["pid"], owned.pid)
                self.assertEqual(os.getpgid(foreign.pid), foreign.pid)
                code, terminated = executor.stop_owned_group(owned)
                self.assertIsNotNone(code)
                self.assertTrue(terminated, "owned wrapper/child group did not terminate")
                with self.assertRaises(ProcessLookupError):
                    os.killpg(owned.pid, 0)
                self.assertIsNone(foreign.poll(), "unrelated own control group was affected")
            finally:
                # These groups were created by this test. Never signal caller PGID.
                for process in (owned, foreign):
                    if process is not None:
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        process.wait(timeout=5)

    def test_full_synthetic_preflight_pins_all94_inputs_and_independent_references(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            repository = Path(__file__).resolve().parents[2]
            def binding(path, data, mode=0o644):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                target.chmod(mode)
                return {"path": path, "bytes": len(data), "mode": oct(mode), "sha256": executor.sha256(data)}
            stream = io.BytesIO()
            with wave.open(stream, "wb") as wav:
                wav.setnchannels(1)
                wav.setsampwidth(2)
                wav.setframerate(16000)
                wav.writeframes(b"\0\0" * 320)
            wav_bytes = stream.getvalue()
            source_cases, cases, inputs = [], [], []
            for index in range(94):
                audio_path = "artifacts/audio/case-%03d.wav" % index
                audio = {"path": audio_path, "sha256": executor.sha256(wav_bytes), "frames": 320, "sampleRate": 16000}
                source = {"id": "case-%03d" % index, "language": "de", "class": "clean", "split": "acceptance", "audio": audio,
                          "reference": "" if index >= 88 else "independent published reference"}
                source_cases.append(source)
                cases.append({k: source[k] for k in ("id", "language", "class", "split", "audio")})
                cases[-1].update(sourceManifest="source.json", referenceUtf8Sha256=executor.sha256(source["reference"].encode()),
                                 expectedOutcome="Empty" if index >= 88 else "Local", warmupRuns=1, measuredRuns=10)
                inputs.append(binding(audio_path, wav_bytes, 0o600))
            manifest = binding("source.json", json.dumps({"cases": source_cases}).encode())
            central = binding("central.json", b'{"cases":[]}')
            selection = {"cases": cases, "sourceManifests": [manifest], "centralManifest": central}
            selection_binding = binding("selection.json", json.dumps(selection).encode())
            sources = [binding("source.rs", b"source")]
            for path in sorted(executor.OWN_SOURCES):
                sources.append(binding(path, (repository / path).read_bytes()))
            model = {"path": "artifacts/models/ggml-large-v3-turbo-q5_0.bin", "id": "large-v3-turbo-q5_0", "sha256": executor.MODEL_SHA, "bytes": 574041195, "mode": "0o644"}
            vad = {"path": "artifacts/models/ggml-silero-v5.1.2.bin", "sha256": executor.VAD_SHA, "bytes": 885098, "mode": "0o600"}
            parameters = {"configurationId": "paced-full-recording-native-timestamps-rtf-tc-owned-v6", "build": {"debugAssertions": False},
                          "runtime": {"fullParams": {"no_timestamps": False, "suppress_nst": False}, "speechGate": {"model": {"sha256": executor.VAD_SHA}}},
                          "measurement": {"chunkMillis": 20, "jitterToleranceNanos": 10000000, "maximumSleepNanos": 1000000,
                                          "finalSpinNanos": 250000, "publisherThread": {"policy": "timeConstrained"}},
                          "models": [{"id": model["id"], "sha256": model["sha256"]}],
                          "sourceHashes": {"source": {"filename": "source.rs", "sha256": sources[0]["sha256"]}}}
            freeze = {"schemaVersion": 1, "kind": "prospective-paced-remainder-v1", "sourceRevision": "a" * 40,
                      "binary": binding("target/release/examples/paced_runtime_probe", b"never executed", 0o755),
                      "parameters": binding("params.json", json.dumps(parameters).encode()), "selection": selection_binding,
                      "sources": sources, "model": model, "vad": vad, "inputs": inputs,
                      "outputDirectory": "artifacts/new", "publisherPolicy": "timeConstrained"}
            freeze_path = root / "freeze.json"
            freeze_path.write_text(json.dumps(freeze))
            real_verify = executor.verify_binding
            def verify_without_models(root_path, bound, fingerprints, maximum=None, read=False):
                # Only external model payloads are substituted; all94 WAV/source/
                # manifest/binary/parameter bytes go through real FD/SHA validation.
                if bound["path"] in (model["path"], vad["path"]):
                    return None
                return real_verify(root_path, bound, fingerprints, maximum, read)
            def git_read(argv, **_kwargs):
                path = argv[-1].split(":", 1)[1]
                data = (root / path).read_bytes()
                return subprocess.CompletedProcess(argv, 0, stdout=str(len(data)).encode() if argv[1] == "cat-file" else data)
            with mock.patch.object(executor, "SELECTION_SHA", selection_binding["sha256"]), mock.patch.object(executor, "verify_binding", side_effect=verify_without_models), mock.patch.object(executor.subprocess, "run", side_effect=git_read):
                plan = executor.preflight(freeze_path, root)
                self.assertEqual(len(plan["cases"]), 94)
                self.assertEqual(len(plan["references"]), 94)
                self.assertEqual(sum(c["expectedOutcome"] == "Empty" for c in plan["cases"]), 6)
                self.assertFalse(plan["output"].exists())
                self.assertEqual(len([p for p in plan["fingerprints"] if p.suffix == ".wav"]), 94)
                extra = binding("additional-reviewed-source.rs", b"also pinned")
                freeze["sources"].append(extra)
                freeze_path.write_text(json.dumps(freeze))
                plan_with_extra = executor.preflight(freeze_path, root)
                self.assertIn(root / extra["path"], plan_with_extra["fingerprints"])
                def different_immutable_extra(argv, **kwargs):
                    if argv[1] == "show" and argv[-1].endswith(":" + extra["path"]):
                        return subprocess.CompletedProcess(argv, 0, stdout=b"other bytes")
                    return git_read(argv, **kwargs)
                with mock.patch.object(executor.subprocess, "run", side_effect=different_immutable_extra), self.assertRaisesRegex(ValueError, "immutable revision"):
                    executor.preflight(freeze_path, root)
                (root / extra["path"]).write_bytes(b"modified")
                with self.assertRaises(ValueError):
                    executor.check_unchanged(plan_with_extra)
                with self.assertRaises(ValueError):
                    executor.preflight(freeze_path, root)
                (root / extra["path"]).write_bytes(b"also pinned")
                freeze["outputDirectory"] = "src/not-ignored"
                freeze_path.write_text(json.dumps(freeze))
                with self.assertRaises(ValueError):
                    executor.preflight(freeze_path, root)
            freeze["outputDirectory"] = "artifacts/new"
            selection["cases"][0]["referenceUtf8Sha256"] = "0" * 64
            freeze["selection"] = binding("selection.json", json.dumps(selection).encode())
            freeze_path.write_text(json.dumps(freeze))
            with mock.patch.object(executor, "SELECTION_SHA", freeze["selection"]["sha256"]), mock.patch.object(executor, "verify_binding", side_effect=verify_without_models), mock.patch.object(executor.subprocess, "run", side_effect=git_read):
                with self.assertRaisesRegex(ValueError, "independent reference SHA"):
                    executor.preflight(freeze_path, root)

    def test_partial_results_keep_observed_rows_and_mark_only_missing_rows_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, case, paths = self.reports(Path(directory))
            results = json.loads(paths["results"].read_text())[:4]
            paths["results"].write_text(json.dumps(results))
            report = executor.validate_case(plan, case, paths)
            self.assertTrue(report["errors"])
            self.assertEqual([r["status"] for r in report["rows"]], ["ok"] * 4 + ["missing"] * 7)

    def test_preflight_failure_never_launches_or_prints_external_text(self):
        stdout = io.StringIO()
        with mock.patch.object(executor, "preflight", side_effect=ValueError("private hypothesis")), mock.patch.object(executor, "launch_case") as launch, contextlib.redirect_stdout(stdout):
            self.assertEqual(executor.main(["frozen.json"]), 2)
        launch.assert_not_called()
        self.assertNotIn("private hypothesis", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
