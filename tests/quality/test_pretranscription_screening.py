import copy
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("pretrans_screen", Path(__file__).resolve().parents[2] / "scripts/run-pretranscription-screening.py")
screen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screen)


def traced_row():
    publications = []
    for i in range(26):
        t = (i + 1) * 20_000_000
        publications.append(dict(startFrame=i*320, endFrame=(i+1)*320,
            plannedNanos=t, beforePublishNanos=t+1, afterPublishNanos=t+10,
            deviationNanos=10, timing="onBudget"))
    jobs = []
    for jid, start, end, early, submit, finish in [
        (1, 0, 3200, True, 500_000_020, 510_000_000),
        (2, 3200, 8320, False, 520_000_020, 530_000_000),
    ]:
        jobs.append(dict(session=1, generation=1, jobId=jid, language="de",
            timelineStart=start, timelineEnd=end, storageStart=0, storageEnd=end-start,
            inputFrames=end-start, requiredStableFrames=end+4800 if early else end,
            audioReleasedAtSubmit=not early, hostSubmitOffsetNanos=submit,
            serviceAcceptedOffsetNanos=submit+1, workerBeforeDecodeOffsetNanos=submit+2,
            workerAfterDecodeOffsetNanos=finish, workerSettledOffsetNanos=finish,
            resultAdoptedOffsetNanos=finish+1, workerOutcome="decoded-ok", adoption="accepted"))
    trace = dict(enabled=True, session=1, maximumJobs=601, jobs=jobs,
                 retiredJob=None, firstOverflowJob=None, error=None, overflowEvents=0)
    evidence = dict(trace=trace, error=None, inputProofs=[
        dict(jobId=1, requiredStableFrames=8000, publishedThroughFramesAtAdmission=8000,
             lastCompletePublicationOffsetNanos=500_000_010),
        dict(jobId=2, requiredStableFrames=8320, publishedThroughFramesAtAdmission=8320,
             lastCompletePublicationOffsetNanos=520_000_010)])
    row = dict(session=1, feed=dict(publications=publications), engineInputFrames=8320,
               engineElapsedNanos=18_000_000, engineRtf=18_000_000/520_000_000,
               tTerminalOffsetNanos=530_000_010)
    return evidence, row


class TraceTests(unittest.TestCase):
    def test_gap_and_future_audio_are_rejected_at_report_boundary(self):
        evidence, row = traced_row()
        screen.validate_trace(evidence, row, 8320, "de")
        for mutate in (lambda e: e["trace"]["jobs"][1].update(timelineStart=3201),
                       lambda e: e["trace"]["jobs"][0].update(hostSubmitOffsetNanos=499_999_999)):
            changed = copy.deepcopy(evidence)
            mutate(changed)
            with self.assertRaises(ValueError):
                screen.validate_trace(changed, row, 8320, "de")


class ExecutionTests(unittest.TestCase):
    def setUp(self):
        from unittest import mock
        self.power = mock.patch.object(screen, "power_snapshot", return_value={"source":"AC Power", "error":None})
        self.power.start()
        self.addCleanup(self.power.stop)

    def test_first_failure_keeps_800_rows_and_never_retries_or_starts_later_blocks(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            blocks = [dict(name=str(b), variant="PretransReference" if b in (0, 3) else "PretransCandidate",
                      cases=[dict(id="case-%03d" % i, output="%d-%03d.json" % (b, i),
                                  metadata="%d-%03d.meta.json" % (b, i), log="%d-%03d.log" % (b, i),
                                  completion="%d-%03d.completion.json" % (b, i)) for i in range(100)]) for b in range(4)]
            calls = []
            def launch(case, paths):
                calls.append(case["id"])
                return dict(exitCode=1, terminationConfirmed=True, launchError="controlled failure")
            report = screen.execute(dict(output=Path(directory)/"new", root=Path(directory).resolve(), blocks=blocks), launch)
            self.assertEqual(calls, ["case-000"])
            self.assertEqual(report["rowCount"], 800)
            rows = [c for b in report["blocks"] for c in b["cases"]]
            self.assertEqual(rows[0]["status"], "failed")
            self.assertEqual([c["status"] for c in rows[1:]], ["unavailable"]*399)
            self.assertFalse(report["completed"])
            self.assertFalse(report["qualified"])

    def test_partial_raw_failure_is_retained_without_repair_or_retry(self):
        import tempfile
        import json
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            blocks = [dict(name=str(b), variant="PretransReference", cases=[dict(id="case-%03d" % i,
                       output="new/%d-%d.json" % (b,i), metadata="new/%d-%d.meta.json" % (b,i),
                       log="new/%d-%d.log" % (b,i), completion="new/%d-%d.done.json" % (b,i))
                       for i in range(100)]) for b in range(4)]
            def launch(case, paths):
                screen.common.write_private(paths["output"], [dict(id="warmup", status="invalid_setup", text=None)])
                screen.common.write_private(paths["metadata"], {"runs": []})
                paths["log"].write_bytes(b"controlled failure")
                paths["log"].chmod(0o600)
                return dict(exitCode=1, terminationConfirmed=True, launchError=None)
            report = screen.execute(dict(output=root/"new", root=root, blocks=blocks), launch)
            first = report["blocks"][0]["cases"][0]
            self.assertEqual(first["rows"][0]["status"], "invalid_setup")
            self.assertEqual(first["rows"][1]["status"], "missing")
            self.assertEqual(json.loads((root/"new/0-0.json").read_bytes())[0]["status"], "invalid_setup")
            self.assertEqual(first["launch"]["exitCode"], 1)
            with self.assertRaises(FileExistsError):
                screen.common.write_private(root/"new/0-0.json", [])
            self.assertEqual((root/"new/0-0.json").stat().st_mode & 0o777, 0o600)


class RetirementTests(unittest.TestCase):
    def test_service_accepted_skip_has_no_invented_decoder_times(self):
        skipped = dict(session=1, generation=1, jobId=1, serviceAcceptedOffsetNanos=4,
                       workerSettledOffsetNanos=9, workerBeforeDecodeOffsetNanos=None,
                       workerAfterDecodeOffsetNanos=None, workerOutcome="cancelled-before-decode", adoption=None)
        screen.validate_retired(skipped, 2)
        bad = dict(skipped, workerAfterDecodeOffsetNanos=8)
        with self.assertRaises(ValueError):
            screen.validate_retired(bad, 2)
        with self.assertRaises(ValueError):
            screen.validate_retired(skipped, 1)


class FreezeTests(unittest.TestCase):
    def test_wrong_order_or_command_cannot_select_a_different_mode(self):
        cases = [dict(id="case-%d" % i, language="de", expectedOutcome="Local", audio={"path": "audio/%d.wav" % i}) for i in range(100)]
        root = Path("/test")
        freeze = dict(outputDirectory="artifacts/new", binary={"path": "target/release/examples/paced_runtime_probe"},
                      parameters={key: {"path": key+".json"} for key in ("PretransReference", "PretransCandidate")}, blocks=[])
        for b, variant in enumerate(("PretransReference", "PretransCandidate", "PretransCandidate", "PretransReference")):
            entries = []
            for i, c in enumerate(cases):
                out = "artifacts/new/%d-%d.json" % (b, i)
                entries.append(dict(id=c["id"], output=out, metadata=out+".metadata.json", log=out+".command.log", completion=out+".completion.json",
                    command=["/usr/bin/time", "-l", "/test/target/release/examples/paced_runtime_probe", "/test/artifacts/models/ggml-large-v3-turbo-q5_0.bin", "reference", "/test/"+c["audio"]["path"], "de", "/test/"+variant+".json", "/test/"+out, "1"]))
            freeze["blocks"].append(dict(name="block-%d" % b, variant=variant, cases=entries))
        screen.validate_blocks(freeze, root, cases)
        freeze["blocks"][1]["cases"][0]["command"][-1] = "10"
        with self.assertRaises(ValueError):
            screen.validate_blocks(freeze, root, cases)


class ProcessTests(unittest.TestCase):
    def test_exit_race_still_reaps_owned_power_child(self):
        class FinishedChild:
            waited = False
            def terminate(self):
                raise ProcessLookupError()
            def wait(self, timeout):
                self.waited = True
                return 0
            def kill(self):
                raise AssertionError("already ended child must not be killed")
        child = FinishedChild()
        self.assertTrue(screen.stop_power(child))
        self.assertTrue(child.waited)


def valid_reports(directory):
    import json
    evidence, basis = traced_row()
    requested = dict(periodTicks=480000, computationTicks=12000, constraintTicks=24000, preemptible=True)
    snapshot = dict(timeConstraint=requested, defaultPolicy=False, basicPolicy=2, cpu=dict(userNanos=1000, systemNanos=2000))
    publisher = dict(policy="timeConstrained", requested=requested, before=dict(snapshot, defaultPolicy=True, basicPolicy=1),
                     configured=snapshot, after=snapshot, beforeFeed=dict(snapshot=snapshot,error=None),
                     afterFeed=dict(snapshot=snapshot,error=None), outliers=[], setupError=None, postError=None, finishError=None)
    rows, results = [], []
    for index, run in enumerate(("warmup", "measured-001")):
        ev = copy.deepcopy(evidence)
        ev["trace"]["session"] = index+1
        for j, proof in zip(ev["trace"]["jobs"],ev["inputProofs"]):
            j["session"] = index+1
            j["jobId"] += 2*index
            proof["jobId"] += 2*index
        row = copy.deepcopy(basis)
        row.update(id=run, warmup=index==0, status="ok", session=index+1, expectedOutcome="Local", terminal="Local", error=None,
                   controllerClockMatchesCapture=True, referenceMatch=None, qosError=None, qosEntered=False, qosFinished=False,
                   publisherJoined=True, publisher=publisher, engineRtfUnavailableReason=None,
                   tStopOffsetNanos=520_000_100, stopToTerminalNanos=9_999_910,
                   tTextOffsetNanos=530_000_010, stopToTextNanos=9_999_910, jobTrace=ev)
        row["feed"].update(timingValid=True, plannedStopOffset=dict(secs=0,nanos=520_000_000),
                            waitSummary=dict(observedPublications=26,detailedPublications=0))
        rows.append(row)
        results.append(dict(id=run,warmup=index==0,status="ok",text="private fixture text"))
    case=dict(id="case",language="de",expectedOutcome="Local",audio=dict(frames=8320,sha256="a"*64))
    plan=dict(parameters={"PretransCandidate":{"test":True}}, parameterBindings={"PretransCandidate":{"sha256":"c"*64}},
              sourceRevision="b"*40, model=dict(id="large-v3-turbo-q5_0",sha256="d"*64))
    meta=dict(hostObservation={"powerSource":"AC Power"},schemaVersion=1,kind="paced-runtime-complete-recording",processId=42,sourceRevision=plan["sourceRevision"],
              fixtureSha256=case["audio"]["sha256"],inputFrames=8320,sampleRate=16000,channels=1,language="de",
              model=plan["model"],parameters={"test":True},parameterDocumentSha256="c"*64,pretranscriptionCandidate=True,
              contextCleanup=None,referenceSha256=None,warmupRuns=1,measuredRunsRequested=1,debugAssertions=False,
              qualified=False,L=None,passed=True,loadError=None,cleanup=dict(joined=True,error=None),expectedOutcome="Local",
              engineTimingCoverage=dict(measuredRuns=1,withActualEngineTiming=1),runs=rows,
              jobTraceAfterCleanup=copy.deepcopy(rows[-1]["jobTrace"]["trace"]),jobTraceAfterCleanupError=None)
    paths=dict(output=directory/"out.json",metadata=directory/"out.json.metadata.json")
    for path,value in ((paths["output"],results),(paths["metadata"],meta)):
        screen.common.write_private(path,value)
    return plan,dict(variant="PretransCandidate"),case,paths,meta


class ReportTests(unittest.TestCase):
    def test_complete_multi_job_report_and_empty_share_actual_engine_contract(self):
        import tempfile
        import json
        with tempfile.TemporaryDirectory() as d:
            plan,block,case,paths,meta=valid_reports(Path(d))
            self.assertEqual(screen.validate_case(plan,block,case,paths)["errors"],[])
            self.assertEqual(screen.validate_case(plan,block,case,paths)["rows"][1]["jobCount"],2)
            empty_meta=copy.deepcopy(meta)
            empty_meta["expectedOutcome"]="Empty"
            for row in empty_meta["runs"]:
                row.update(expectedOutcome="Empty",terminal="Empty",tTextOffsetNanos=None,stopToTextNanos=None)
            empty_results=[dict(id=run,warmup=i==0,status="ok",text="") for i,run in enumerate(("warmup","measured-001"))]
            paths["output"].write_text(json.dumps(empty_results))
            paths["metadata"].write_text(json.dumps(empty_meta))
            self.assertEqual(screen.validate_case(plan,block,dict(case,expectedOutcome="Empty"),paths)["errors"],[])
            paths["output"].write_text(json.dumps([dict(id=run,warmup=i==0,status="ok",text="private fixture text") for i,run in enumerate(("warmup","measured-001"))]))
            meta["runs"][1]["engineInputFrames"] = 8321
            paths["metadata"].write_text(json.dumps(meta))
            self.assertTrue(screen.validate_case(plan,block,case,paths)["errors"])

    def test_post_join_missing_or_old_trace_and_failed_warmup_are_not_hidden(self):
        import tempfile
        import json
        with tempfile.TemporaryDirectory() as d:
            plan,block,case,paths,meta=valid_reports(Path(d))
            for change in (lambda m: m.update(jobTraceAfterCleanup=None),
                           lambda m: m["jobTraceAfterCleanup"].update(session=1),
                           lambda m: m["runs"][0].update(error="warmup failure",status="failed")):
                invalid=copy.deepcopy(meta)
                change(invalid)
                paths["metadata"].write_text(json.dumps(invalid))
                self.assertTrue(screen.validate_case(plan,block,case,paths)["errors"])


class OwnedProcessTests(unittest.TestCase):
    def test_real_nonzero_child_is_reaped_and_private_log_is_retained(self):
        import sys
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            paths={"log":root/"child.log"}
            entry={"id":"controlled-child", "command":[sys.executable,"-c","print('controlled child'); raise SystemExit(3)"]}
            result=screen.launch_case(dict(root=root,sourceRevision="a"*40),entry,paths)
            self.assertEqual(result["exitCode"],3)
            self.assertTrue(result["terminationConfirmed"])
            self.assertIsNone(result["launchError"])
            self.assertEqual(paths["log"].stat().st_mode & 0o777,0o600)
            self.assertEqual(paths["log"].read_text().strip(),"controlled child")
            self.assertIsNone(screen._ACTIVE)


class PowerAdmissionTests(unittest.TestCase):
    def test_battery_rejects_before_output_directory_or_any_launch(self):
        import tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            blocks=[dict(name=str(b),variant="PretransReference",cases=[dict(id="case-%d"%i,
                         output="new/%d-%d.json"%(b,i),metadata="new/%d-%d.meta"%(b,i),
                         log="new/%d-%d.log"%(b,i),completion="new/%d-%d.done"%(b,i)) for i in range(100)]) for b in range(4)]
            launch=mock.Mock(return_value={"exitCode":1,"terminationConfirmed":True})
            with mock.patch.object(screen,"power_snapshot",return_value={"source":"Battery Power","error":None},create=True):
                with self.assertRaisesRegex(ValueError,"AC Power"):
                    screen.execute(dict(root=root,output=root/"new",blocks=blocks),launch)
            launch.assert_not_called()
            self.assertFalse((root/"new").exists())

    def test_probe_host_battery_is_rejected_even_with_valid_pipeline(self):
        import tempfile
        import json
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta=valid_reports(Path(directory))
            meta["hostObservation"]={"powerSource":"Battery Power"}
            paths["metadata"].write_text(json.dumps(meta))
            result=screen.validate_case(plan,block,case,paths)
            self.assertTrue(result["errors"])
            self.assertIn("AC",result["errors"][0])


class PowerObservationTests(unittest.TestCase):
    def test_pmset_source_is_authoritative_and_failure_is_not_ac(self):
        from unittest import mock
        for output,source,error in [
            ("Now drawing from 'AC Power'\n-Battery\n", "AC Power",None),
            ("Now drawing from 'Battery Power'\nlast source AC Power\n", "Battery Power",None),
            ("unrecognized AC Power",None,"pmset power source unavailable"),
        ]:
            with mock.patch.object(screen,"read_power_output",return_value=dict(stdout=output,error=None,exitCode=0,timedOut=False,outputBytes=len(output))):
                snap=screen.power_snapshot()
            self.assertEqual(snap["source"],source)
            self.assertEqual(snap["error"],error)
            self.assertEqual(snap["command"],["/usr/bin/pmset","-g","batt"])
        with mock.patch.object(screen,"read_power_output",return_value=dict(stdout=None,error="pmset read timed out",exitCode=-9,timedOut=True,outputBytes=0)):
            snap=screen.power_snapshot()
            with self.assertRaises(screen.PowerAdmissionError):
                screen.require_ac(snap)

    def test_after_probe_battery_preserves_current_result_and_blocks_every_later_launch(self):
        import tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            blocks=[dict(name=str(b),variant="PretransReference",cases=[dict(id="case-%d"%i,
                         output="new/%d-%d.json"%(b,i),metadata="new/%d-%d.meta"%(b,i),
                         log="new/%d-%d.log"%(b,i),completion="new/%d-%d.done"%(b,i)) for i in range(100)]) for b in range(4)]
            ac={"source":"AC Power","error":None}
            battery={"source":"Battery Power","error":None}
            calls=[]
            def launch(case,paths):
                calls.append(case["id"])
                for key in ("output","metadata"):
                    screen.common.write_private(paths[key],{})
                paths["log"].write_bytes(b"")
                paths["log"].chmod(0o600)
                return dict(exitCode=0,terminationConfirmed=True)
            validation=dict(errors=[],rows=[dict(status="ok"),dict(status="ok")],processId=1)
            with mock.patch.object(screen,"power_snapshot",side_effect=[ac,ac,battery]), mock.patch.object(screen,"validate_case",return_value=validation):
                report=screen.execute(dict(root=root,output=root/"new",blocks=blocks),launch)
            self.assertEqual(calls,["case-0"])
            entries=[c for b in report["blocks"] for c in b["cases"]]
            self.assertEqual(entries[0]["beforePower"],ac)
            self.assertEqual(entries[0]["afterPower"],battery)
            self.assertEqual(entries[0]["rows"],validation["rows"])
            self.assertIn("post-probe AC Power",entries[0]["error"])
            self.assertTrue(all(c["status"]=="unavailable" for c in entries[1:]))

    def test_saved_v1_case_one_battery_observation_is_rejected(self):
        import tempfile
        import json
        import hashlib
        saved=Path(__file__).resolve().parents[2]/"artifacts/p06-pretranscription-screening/v1/01-reference-001-fleurs-de-test-1660.json.metadata.json"
        if not saved.exists():
            self.skipTest("private immutable v1 raw evidence unavailable")
        data=screen.common.bounded_bytes(saved,screen.common.MAX_METADATA)
        self.assertEqual(hashlib.sha256(data).hexdigest(),"77d79c5cf50cfad44ab68ec20e963e4e1c29206b69123a5c21bd8b0b93bdf21b")
        observation=json.loads(data)["hostObservation"]
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta=valid_reports(Path(directory))
            meta["hostObservation"]=observation
            paths["metadata"].write_text(json.dumps(meta))
            self.assertIn("AC Power",screen.validate_case(plan,block,case,paths)["errors"][0])


class PowerBoundaryTests(unittest.TestCase):
    def test_main_battery_admission_precedes_caffeinate_and_output(self):
        from unittest import mock
        import contextlib
        import io
        battery={"source":"Battery Power","error":None}
        with mock.patch.object(screen,"preflight",return_value={}), mock.patch.object(screen,"power_snapshot",return_value=battery), mock.patch.object(screen.subprocess,"Popen") as start, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(screen.main(["freeze.json","--execute"]),2)
        start.assert_not_called()
        with mock.patch.object(screen,"preflight",return_value={}), mock.patch.object(screen,"power_snapshot",side_effect=AssertionError("binding-only preflight must not observe power")), contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(screen.main(["freeze.json"]),0)
            self.assertIn("not checked",out.getvalue())

    def test_bounded_pmset_timeout_kills_and_reaps_and_oversize_is_unavailable(self):
        from unittest import mock
        class Child:
            def __init__(self, completed):
                self.completed=completed
                self.killed=False
                self.waited=False
            def poll(self):
                return 0 if self.completed else None
            def kill(self):
                self.killed=True
                self.completed=True
            def wait(self,timeout):
                self.waited=True
                return -9 if self.killed else 0
        held=Child(False)
        with mock.patch.object(screen.subprocess,"Popen",return_value=held), mock.patch.object(screen.time,"monotonic",side_effect=[0,3]):
            result=screen.read_power_output()
        self.assertTrue(held.killed and held.waited)
        self.assertTrue(result["timedOut"])
        self.assertIsNone(result["stdout"])
        complete=Child(True)
        def oversized(command,stdout,stderr):
            stdout.write(b"x"*(screen.POWER_MAX_BYTES+1))
            stdout.flush()
            return complete
        with mock.patch.object(screen.subprocess,"Popen",side_effect=oversized):
            result=screen.read_power_output()
        self.assertEqual(result["error"],"pmset output limit exceeded")
        self.assertIsNone(result["stdout"])
        self.assertTrue(complete.waited)

    def test_case_admission_rechecks_ac_after_startup_and_never_launches_on_unknown(self):
        import tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            blocks=[dict(name=str(b),variant="PretransReference",cases=[dict(id="case-%d"%i,
                         output="new/%d-%d.json"%(b,i),metadata="new/%d-%d.meta"%(b,i),
                         log="new/%d-%d.log"%(b,i),completion="new/%d-%d.done"%(b,i)) for i in range(100)]) for b in range(4)]
            unavailable={"source":None,"error":"pmset read timed out"}
            launch=mock.Mock()
            with mock.patch.object(screen,"power_snapshot",side_effect=[{"source":"AC Power","error":None},unavailable]):
                report=screen.execute(dict(root=root,output=root/"new",blocks=blocks),launch)
            launch.assert_not_called()
            first=report["blocks"][0]["cases"][0]
            self.assertEqual(first["beforePower"],unavailable)
            self.assertIsNone(first["afterPower"])
            self.assertEqual(first["status"],"failed")
            self.assertFalse(report["completed"])
