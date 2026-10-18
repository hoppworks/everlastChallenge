import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile
import json
import os
import sys
import subprocess
import io
import wave
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("full_prefix_screen", ROOT / "scripts/run-full-prefix-screening.py")
screen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screen)


def trace_fixture():
    publications = []
    for index in range(50):
        at = (index + 1) * 20_000_000
        publications.append(dict(startFrame=index*320, endFrame=(index+1)*320,
            plannedNanos=at, beforePublishNanos=at, afterPublishNanos=at+1,
            deviationNanos=1, timing="onBudget"))
    jobs, proofs = [], []
    for jid, end, final, submit, available in [(11,6400,False,700_000_010,11200), (12,16000,True,1_000_000_010,16000)]:
        receipt = dict(session=1, generation=1, jobId=jid,
            outcome="hit" if final else "decoded", originJobId=11,
            preparedFrames=16000, preparedPcmSha256=[17]*32,
            decoderInvoked=not final, nativeFullAttempted=not final)
        jobs.append(dict(session=1, generation=1, jobId=jid, language="de",
            timelineStart=0, timelineEnd=end, storageStart=0, storageEnd=end,
            inputFrames=end, audioReleasedAtSubmit=final,
            requiredStableFrames=end if final else end+4800,
            hostSubmitOffsetNanos=submit, serviceAcceptedOffsetNanos=submit+1,
            workerBeforeDecodeOffsetNanos=submit+2, workerAfterDecodeOffsetNanos=submit+100_000,
            workerSettledOffsetNanos=submit+100_001, completionObservedOffsetNanos=submit+100_002,
            resultAdoptedOffsetNanos=submit+100_003, workerOutcome="decoded-ok", adoption="accepted",
            preparedReuse=receipt))
        proofs.append(dict(jobId=jid, requiredStableFrames=end if final else end+4800,
            publishedThroughFramesAtAdmission=available))
    trace = dict(enabled=True, session=1, maximumJobs=601, jobs=jobs, retiredJob=None,
        firstOverflowJob=None, error=None, overflowEvents=0)
    evidence = dict(trace=trace, inputProofs=proofs, processedFramesSum=22400,
        checkedWorkerSpanSumNanos=199996,finalAdoptionOffsetNanos=1_000_100_013,error=None)
    row = dict(session=1, jobTrace=evidence, feed=dict(publications=publications),
        engineInputFrames=22400, engineElapsedNanos=100_000, engineRtf=.0001,
        sessionRtfRecordingFrames=16000, tTerminalOffsetNanos=1_000_100_013)
    return evidence, row


class TraceTests(unittest.TestCase):
    def test_overlapping_prefixes_use_unique_recording_denominator(self):
        evidence, row = trace_fixture()
        summary = screen.validate_trace(evidence, row, 16000, "de", True)
        self.assertEqual(summary.get("processedFramesSum"), 22400)
        self.assertEqual(summary.get("recordingFrames"), 16000)
        self.assertEqual(summary.get("engineRtf"), .0001)

    def test_boolean_zero_cannot_impersonate_a_frame_index(self):
        evidence, row = trace_fixture()
        evidence["trace"]["jobs"][0]["timelineStart"] = False
        with self.assertRaises(ValueError):
            screen.validate_trace(evidence, row, 16000, "de", True)

    def test_tampered_overlap_receipt_availability_and_timing_are_rejected(self):
        mutations = [
            lambda e,r:e["trace"]["jobs"][0].update(requiredStableFrames=0),
            lambda e,r:e["trace"]["jobs"][0].update(hostSubmitOffsetNanos=None),
            lambda e,r:e["trace"]["jobs"][1].update(timelineStart=6400),
            lambda e,r:e["trace"]["jobs"][1].update(audioReleasedAtSubmit=False),
            lambda e,r:e["trace"]["jobs"][0].update(completionObservedOffsetNanos=None),
            lambda e,r:e["trace"]["jobs"][1].update(resultAdoptedOffsetNanos=r["tTerminalOffsetNanos"]+1),
            lambda e,r:e["trace"]["jobs"][0].update(preparedReuse=None),
            lambda e,r:e["trace"]["jobs"][0]["preparedReuse"].update(nativeFullAttempted=False),
            lambda e,r:e["trace"]["jobs"][1]["preparedReuse"].update(session=2),
            lambda e,r:e["trace"]["jobs"][1]["preparedReuse"].update(originJobId=12),
            lambda e,r:e["trace"]["jobs"][1]["preparedReuse"].update(preparedPcmSha256=[18]*32),
            lambda e,r:e.update(processedFramesSum=16000),
            lambda e,r:r.update(engineInputFrames=16000),
            lambda e,r:r.update(sessionRtfRecordingFrames=22400),
            lambda e,r:r.update(engineRtf=100_000*16000/22400/1e9),
            lambda e,r:r.update(engineElapsedNanos=1_000_000,engineRtf=.001),
            lambda e,r:r["feed"]["publications"][34].update(afterPublishNanos=700_000_011),
        ]
        for index, mutate in enumerate(mutations):
            evidence,row = trace_fixture()
            mutate(evidence,row)
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                screen.validate_trace(evidence,row,16000,"de",True)

    def test_last_entry_eviction_and_nonadmission_invalidate_old_origins(self):
        for nonadmitted in (False,True):
            evidence,row = trace_fixture()
            middle = copy.deepcopy(evidence["trace"]["jobs"][0])
            middle.update(jobId=12,timelineEnd=9600,storageEnd=9600,inputFrames=9600,requiredStableFrames=14400)
            for key in ("hostSubmitOffsetNanos","serviceAcceptedOffsetNanos","workerBeforeDecodeOffsetNanos",
                        "workerAfterDecodeOffsetNanos","workerSettledOffsetNanos","completionObservedOffsetNanos","resultAdoptedOffsetNanos"):
                middle[key] += 200_000_000
            middle["preparedReuse"].update(jobId=12,originJobId=12,preparedPcmSha256=[18]*32)
            if nonadmitted:
                middle["preparedReuse"].update(outcome="not-admitted",originJobId=None,preparedFrames=None,
                    preparedPcmSha256=None,decoderInvoked=False,nativeFullAttempted=False)
            evidence["trace"]["jobs"][-1]["jobId"] = 13
            evidence["trace"]["jobs"][-1]["preparedReuse"]["jobId"] = 13
            evidence["trace"]["jobs"].insert(1,middle)
            evidence["inputProofs"].insert(1,dict(jobId=12,requiredStableFrames=14400,publishedThroughFramesAtAdmission=14400))
            evidence["inputProofs"][-1]["jobId"] = 13
            evidence["processedFramesSum"] = row["engineInputFrames"] = 32000
            evidence["checkedWorkerSpanSumNanos"] = 299994
            with self.subTest(nonadmitted=nonadmitted), self.assertRaises(ValueError):
                screen.validate_trace(evidence,row,16000,"de",True)


def execution_plan(root):
    output = root / "new"
    blocks = []
    for index, variant in enumerate(screen.VARIANTS):
        cases = []
        for number in range(100):
            stem = "new/%d-%03d" % (index, number)
            cases.append(dict(id="case-%03d" % number, output=stem+".json", metadata=stem+".json.metadata.json",
                log=stem+".log", completion=stem+".completion.json"))
        blocks.append(dict(name=str(index),variant=variant,cases=cases))
    return dict(root=root,output=output,blocks=blocks,screeningId="fixed-test-only",sourceRevision="a"*40)


class ExecutionTests(unittest.TestCase):
    def test_first_failure_retains_800_slots_and_never_launches_again(self):
        with tempfile.TemporaryDirectory() as directory:
            plan, calls = execution_plan(Path(directory).resolve()), []
            def launch(case, paths):
                calls.append(case["id"])
                return dict(exitCode=3,terminationConfirmed=True,launchError=None)
            with mock.patch.object(screen.process,"power_snapshot",return_value={"source":"AC Power","error":None}):
                report = screen.execute(plan,launch)
            self.assertEqual(calls,["case-000"])
            self.assertFalse(report["completed"])
            self.assertEqual(report["rowCount"],800)
            entries = [case for block in report["blocks"] for case in block["cases"]]
            self.assertEqual(entries[0]["status"],"failed")
            self.assertEqual([case["status"] for case in entries[1:]],["unavailable"]*399)

    def test_startup_battery_blocks_directory_and_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            plan = execution_plan(Path(directory).resolve())
            with mock.patch.object(screen.process,"power_snapshot",return_value={"source":"Battery Power","error":None}):
                launch = mock.Mock()
                with self.assertRaises(screen.process.PowerAdmissionError):
                    screen.execute(plan,launch)
                launch.assert_not_called()
            self.assertFalse(plan["output"].exists())

    def test_real_owned_exit_three_and_private_no_clobber_completions(self):
        with tempfile.TemporaryDirectory() as directory:
            plan = execution_plan(Path(directory).resolve())
            plan["blocks"][0]["cases"][0]["command"] = [sys.executable,"-c","raise SystemExit(3)"]
            with mock.patch.object(screen.process,"power_snapshot",return_value={"source":"AC Power","error":None}):
                report = screen.execute(plan,lambda case,paths:screen.process.launch_case(plan,case,paths))
            first = report["blocks"][0]["cases"][0]
            self.assertEqual(first["launch"]["exitCode"],3)
            self.assertTrue(first["launch"]["terminationConfirmed"])
            self.assertEqual(first["status"],"failed")
            self.assertEqual(len(list(plan["output"].glob("*.completion.json"))),400)
            self.assertTrue(all((path.stat().st_mode & 0o777) == 0o600 for path in plan["output"].iterdir()))
            with self.assertRaises(FileExistsError):
                screen.common.write_private(plan["output"]/"completion.json",{})

    def test_complete_400case_loop_and_postcase_power_failure_keep_exact_slots(self):
        for power_failure in (False,True):
            with self.subTest(power_failure=power_failure), tempfile.TemporaryDirectory() as directory:
                plan,calls=execution_plan(Path(directory).resolve()),[]
                def launch(case,paths):
                    calls.append(case["id"])
                    for key in ("output","metadata","log"):
                        screen.common.write_private(paths[key],{})
                    return dict(exitCode=0,terminationConfirmed=True,launchError=None)
                def validate(_plan,_block,case,_paths):
                    return dict(errors=[],rows=screen.process.canonical_rows(case,"ok"),processId=2000+len(calls))
                ac=dict(source="AC Power",error=None)
                battery=dict(source="Battery Power",error=None)
                power_options={"side_effect":[ac,ac,battery]} if power_failure else {"return_value":ac}
                with mock.patch.object(screen,"validate_case",side_effect=validate), \
                     mock.patch.object(screen.process,"power_snapshot",**power_options):
                    report=screen.execute(plan,launch,lambda:True)
                self.assertEqual(len(calls),1 if power_failure else 400)
                self.assertEqual(report["rowCount"],800)
                self.assertEqual(report["completed"],not power_failure)
                self.assertFalse(report["qualified"])
                self.assertIsNone(report["L"])
                self.assertTrue(report["powerAssertionEnded"])
                entries=[case for block in report["blocks"] for case in block["cases"]]
                self.assertEqual([case["globalSlot"] for case in entries],list(range(1,401)))
                if power_failure:
                    self.assertEqual(entries[0]["afterPower"]["source"],"Battery Power")
                    self.assertEqual([case["status"] for case in entries[1:]],["unavailable"]*399)


def block_fixture(root):
    plan = execution_plan(root)
    cases = [dict(id="case-%03d" % number,language="de",expectedOutcome="Local",
                  audio=dict(path="audio/%03d.wav" % number)) for number in range(100)]
    freeze = dict(outputDirectory="new",binary=dict(path="target/release/examples/paced_runtime_probe"),
        parameters={variant:dict(path="params-"+variant+".json") for variant in set(screen.VARIANTS)},blocks=plan["blocks"])
    for block in freeze["blocks"]:
        for entry,case in zip(block["cases"],cases):
            entry["command"] = ["/usr/bin/time","-l",str(root/freeze["binary"]["path"]),str(root/screen.process.MODEL_PATH),
                "reference",str(root/case["audio"]["path"]),"de",str(root/freeze["parameters"][block["variant"]]["path"]),str(root/entry["output"]),"1"]
    return freeze,cases


class PlanTests(unittest.TestCase):
    def test_global_sidecar_names_are_not_case_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            freeze,cases = block_fixture(root)
            freeze["blocks"][0]["cases"][0]["completion"] = "new/completion.json"
            with self.assertRaises(ValueError):
                screen.validate_blocks(freeze,root,cases)

    def test_frozen_400_commands_forbid_extra_runs_old_arms_and_output_reuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            for mutate in (
                lambda f:f["blocks"][0].update(variant="PretransReference"),
                lambda f:f["blocks"][0]["cases"][0]["command"].__setitem__(-1,"30"),
                lambda f:f["blocks"][0]["cases"][1].update(completion=f["blocks"][0]["cases"][0]["completion"]),
                lambda f:f["blocks"][0]["cases"].pop(),
            ):
                freeze,cases = block_fixture(root)
                mutate(freeze)
                with self.assertRaises(ValueError):
                    screen.validate_blocks(freeze,root,cases)

    def test_parameter_modes_are_explicit_full_prefix_not_legacy_or_disjoint(self):
        for candidate in (False,True):
            variant="FullPrefixCandidate" if candidate else "FullPrefixReference"
            document=parameter_fixture(candidate)
            screen.validate_parameters(document,variant)
            for mutate in (lambda d:d.update(configurationId="p06-pretranscription-candidate-v1"),
                           lambda d:d.update(pretranscription=dict(candidate=candidate)),
                           lambda d:d["measurement"].update(mode="paced-complete-recording"),
                           lambda d:d["fullPrefix"].update(candidate=not candidate),
                           lambda d:d["runtime"]["fullParams"].update(audio_ctx=1024)):
                changed=copy.deepcopy(document)
                mutate(changed)
                with self.assertRaises(ValueError):screen.validate_parameters(changed,variant)


def valid_reports(directory):
    rows,results = [],[]
    requested = dict(periodTicks=480000,computationTicks=12000,constraintTicks=24000,preemptible=True)
    snapshot = dict(timeConstraint=requested,defaultPolicy=False,basicPolicy=2,cpu=dict(userNanos=1000,systemNanos=2000))
    publisher = dict(policy="timeConstrained",requested=requested,before=dict(snapshot,defaultPolicy=True,basicPolicy=1),
        configured=snapshot,after=snapshot,beforeFeed=dict(snapshot=snapshot,error=None),afterFeed=dict(snapshot=snapshot,error=None),
        outliers=[],setupError=None,postError=None,finishError=None)
    for index,run in enumerate(("warmup","measured-001")):
        evidence,row = trace_fixture()
        evidence["trace"]["session"] = row["session"] = index+1
        for job,proof in zip(evidence["trace"]["jobs"],evidence["inputProofs"]):
            job["session"] = index+1
            job["jobId"] += index*2
            job["preparedReuse"]["session"] = index+1
            job["preparedReuse"]["jobId"] += index*2
            job["preparedReuse"]["originJobId"] += index*2
            proof["jobId"] += index*2
        row.update(id=run,warmup=index==0,status="ok",error=None,expectedOutcome="Local",terminal="Local",
            controllerClockMatchesCapture=True,referenceMatch=None,qosError=None,qosEntered=False,qosFinished=False,
            publisherJoined=True,publisher=publisher,engineRtfUnavailableReason=None,tStopOffsetNanos=1_000_000_010,
            stopToTerminalNanos=100_003,tTextOffsetNanos=1_000_100_013,stopToTextNanos=100_003)
        row["feed"].update(timingValid=True,plannedStopOffset=dict(secs=1,nanos=0),
                           waitSummary=dict(observedPublications=50,detailedPublications=0))
        rows.append(row)
        results.append(dict(id=run,warmup=index==0,status="ok",text="controlled public fixture"))
    case = dict(id="case",language="de",expectedOutcome="Local",audio=dict(frames=16000,sha256="a"*64))
    plan = dict(parameters={"FullPrefixCandidate":{"test":True}},parameterBindings={"FullPrefixCandidate":{"sha256":"c"*64}},
        sourceRevision="b"*40,model=dict(id="large-v3-turbo-q5_0",sha256="d"*64))
    meta = dict(schemaVersion=1,kind="paced-runtime-full-prefix-revision-v1",processId=42,hostObservation=dict(powerSource="AC Power"),
        sourceRevision=plan["sourceRevision"],fixtureSha256=case["audio"]["sha256"],inputFrames=16000,sampleRate=16000,channels=1,
        language="de",model=plan["model"],parameters={"test":True},parameterDocumentSha256="c"*64,
        fullPrefixCandidate=True,pretranscriptionCandidate=None,contextCleanup=None,referenceSha256=None,
        warmupRuns=1,measuredRunsRequested=1,debugAssertions=False,qualified=False,L=None,passed=True,loadError=None,
        cleanup=dict(joined=True,error=None),expectedOutcome="Local",engineTimingCoverage=dict(measuredRuns=1,withActualEngineTiming=1),
        runs=rows,jobTraceAfterCleanup=copy.deepcopy(rows[-1]["jobTrace"]["trace"]),jobTraceAfterCleanupError=None)
    paths = dict(output=directory/"out.json",metadata=directory/"out.json.metadata.json")
    for path,value in ((paths["output"],results),(paths["metadata"],meta)):
        screen.common.write_private(path,value)
    return plan,dict(variant="FullPrefixCandidate"),case,paths,meta


class ReportTests(unittest.TestCase):
    def test_actual_two_phase_shape_and_changed_foreign_receipt_are_not_old_disjoint_reports(self):
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta = valid_reports(Path(directory))
            with mock.patch.object(screen.process,"validate_trace",side_effect=AssertionError("old disjoint validator called")):
                validated = screen.validate_case(plan,block,case,paths)
            self.assertEqual(validated["errors"],[])
            self.assertEqual([row["processedFramesSum"] for row in validated["rows"]],[22400,22400])
            self.assertEqual([row["recordingFrames"] for row in validated["rows"]],[16000,16000])
            for mutate in (
                lambda m:m.update(kind="paced-runtime-complete-recording"),
                lambda m:m.update(pretranscriptionCandidate=True),
                lambda m:m.update(fullPrefixCandidate=False),
                lambda m:m.update(jobTraceAfterCleanupError="controlled late failure"),
                lambda m:m["runs"][1]["jobTrace"]["trace"]["jobs"][0]["preparedReuse"].update(session=1),
                lambda m:m.update(hostObservation=dict(powerSource="Battery Power")),
            ):
                changed = copy.deepcopy(meta)
                mutate(changed)
                paths["metadata"].write_text(json.dumps(changed))
                self.assertTrue(screen.validate_case(plan,block,case,paths)["errors"])

    def test_reference_empty_is_explicit_nonadmission_without_native_or_text_time(self):
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta=valid_reports(Path(directory))
            block["variant"]="FullPrefixReference"
            plan["parameters"][block["variant"]]=plan["parameters"].pop("FullPrefixCandidate")
            plan["parameterBindings"][block["variant"]]=plan["parameterBindings"].pop("FullPrefixCandidate")
            meta.update(fullPrefixCandidate=False,expectedOutcome="Empty")
            case["expectedOutcome"]="Empty"
            results=[]
            for row in meta["runs"]:
                row.update(expectedOutcome="Empty",terminal="Empty",tTextOffsetNanos=None,stopToTextNanos=None,
                    engineInputFrames=16000,engineElapsedNanos=30000,engineRtf=.00003)
                evidence=row["jobTrace"]
                evidence["trace"]["jobs"]=evidence["trace"]["jobs"][-1:]
                evidence["inputProofs"]=evidence["inputProofs"][-1:]
                evidence.update(processedFramesSum=16000,checkedWorkerSpanSumNanos=99998)
                evidence["trace"]["jobs"][0]["preparedReuse"].update(outcome="not-admitted",originJobId=None,
                    preparedFrames=None,preparedPcmSha256=None,decoderInvoked=False,nativeFullAttempted=False)
                results.append(dict(id=row["id"],warmup=row["warmup"],status="ok",text=""))
            meta["jobTraceAfterCleanup"]=copy.deepcopy(meta["runs"][-1]["jobTrace"]["trace"])
            paths["metadata"].write_text(json.dumps(meta))
            paths["output"].write_text(json.dumps(results))
            self.assertEqual(screen.validate_case(plan,block,case,paths)["errors"],[])
            meta["runs"][0]["jobTrace"]["trace"]["jobs"][0]["preparedReuse"]["preparedFrames"]=16000
            paths["metadata"].write_text(json.dumps(meta))
            self.assertTrue(screen.validate_case(plan,block,case,paths)["errors"])

    def test_postjoin_receipt_cannot_replace_boolean_true_with_integer_one(self):
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta=valid_reports(Path(directory))
            meta["jobTraceAfterCleanup"]["jobs"][0]["preparedReuse"]["nativeFullAttempted"]=1
            paths["metadata"].write_text(json.dumps(meta))
            self.assertTrue(screen.validate_case(plan,block,case,paths)["errors"])


def parameter_fixture(candidate):
    return dict(configurationId="p06-full-prefix-"+("candidate" if candidate else "reference")+"-v1",
        build=dict(debugAssertions=False),runtime=dict(context=dict(flash_attn=False),
            fullParams=dict(audio_ctx=0,no_timestamps=False,suppress_nst=False)),
        fullPrefix=dict(candidate=candidate,windowFrames=320,startRms=.00005,endRms=.000025,prerollFrames=2400,
            tailFrames=3200,confirmationFrames=8000,maximumJobs=601,qualification=False),
        measurement=dict(mode="paced-full-prefix-revision-v1",chunkMillis=20,jitterToleranceNanos=10_000_000,
            maximumSleepNanos=1_000_000,finalSpinNanos=250_000,publisherThread=dict(policy="timeConstrained")))


def synthetic_preflight_fixture(root):
    """Real files/Git/print-only subprocess; substitute only canonical-corpus/model boundaries.

    This fixture has no model, ASR or actual compiled Rust binary. It tests the
    launcher's structural preflight, not the actual fixed100/binary acceptance.
    """
    def write(path,data,mode=0o600):
        target = root/path
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(data)
        target.chmod(mode)
        return dict(path=path,bytes=len(data),sha256=screen.common.sha256(data),mode=oct(mode))
    def document(path,value):
        return write(path,json.dumps(value).encode())
    sources = [write(path,(ROOT/path).read_bytes()) for path in sorted(screen.OWN_SOURCES)]
    compiled = write("src-tauri/src/runtime.rs",b"// synthetic compiled provenance boundary only\n")
    sources.append(compiled)
    params = {variant:parameter_fixture(variant=="FullPrefixCandidate") for variant in set(screen.VARIANTS)}
    for value in params.values():
        value["sourceHashes"] = dict(runtime=dict(filename=compiled["path"],sha256=compiled["sha256"]))
    parameter_bindings = {variant:document("params-"+variant+".json",value) for variant,value in params.items()}
    printed = {"--print-full-prefix-"+("candidate" if v=="FullPrefixCandidate" else "reference")+"-parameters":json.dumps(value)
               for v,value in params.items()}
    printer = ("#!"+sys.executable+"\nimport sys\nvalues="+repr(printed)+"\n"
               "if len(sys.argv)!=2 or sys.argv[1] not in values: raise SystemExit(3)\n"
               "sys.stdout.write(values[sys.argv[1]])\n").encode()
    binary = write("target/release/examples/paced_runtime_probe",printer,0o755)
    with io.BytesIO() as output:
        with wave.open(output,"wb") as wav:
            wav.setparams((1,2,16000,320,"NONE","not compressed"))
            wav.writeframes(b"\0\0"*320)
        wav_bytes = output.getvalue()
    cases,manifest,inputs = [],[],[]
    for number in range(100):
        binding = write("artifacts/input/%03d.wav" % number,wav_bytes)
        audio = dict(path=binding["path"],sha256=binding["sha256"],sampleRate=16000,frames=320)
        reference = "" if number<6 else "controlled public reference"
        case = dict(id="case-%03d" % number,language="de" if number%2 else "en",split="acceptance",**{"class":"clean"},audio=audio)
        manifest.append(dict(case,reference=reference))
        cases.append(dict(case,referenceUtf8Sha256=screen.common.sha256(reference.encode()),expectedOutcome="Local" if reference else "Empty"))
        inputs.append(binding)
    selection = document("selection.json",dict(cases=cases))
    manifest_binding = document("manifest.json",dict(unicodeVersion="13.0.0",cases=manifest))
    freeze,unused = block_fixture(root)
    freeze.update(schemaVersion=1,kind=screen.KIND,screeningId="424273c0-04bc-4cdc-a30e-cfa2cec53d7a",
        binary=binary,selection=selection,manifest=manifest_binding,parameters=parameter_bindings,
        sources=sources,launcher=next(source for source in sources if source["path"]=="scripts/run-full-prefix-screening.py"),
        models=[dict(path=screen.process.MODEL_PATH,sha256=screen.common.MODEL_SHA,bytes=574041195,mode="0o600"),
                dict(path=screen.process.VAD_PATH,sha256=screen.common.VAD_SHA,bytes=885098,mode="0o600")],
        inputs=inputs,publisherPolicy="timeConstrained",caseTimeoutSeconds=7200,qualified=False,L=None,outputDirectory="artifacts/new")
    for block in freeze["blocks"]:
        for number,(entry,case) in enumerate(zip(block["cases"],cases)):
            stem="artifacts/new/"+block["name"]+"-%03d" % number
            entry.update(output=stem+".json",metadata=stem+".json.metadata.json",log=stem+".log",completion=stem+".completion.json")
            entry["command"]=["/usr/bin/time","-l",str(root/binary["path"]),str(root/screen.process.MODEL_PATH),"reference",
                str(root/case["audio"]["path"]),case["language"],str(root/parameter_bindings[block["variant"]]["path"]),str(root/entry["output"]),"1"]
            if case["expectedOutcome"]=="Empty": entry["command"].append("--expect-empty")
    for command in (["git","init","-q"],["git","add",*sorted(source["path"] for source in sources)],
                    ["git","-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-q","-m","structural fixture"]):
        subprocess.run(command,cwd=root,check=True,capture_output=True)
    freeze["sourceRevision"] = subprocess.check_output(["git","rev-parse","HEAD"],cwd=root).decode().strip()
    document("freeze.json",freeze)
    return freeze,selection["sha256"],manifest_binding["sha256"]


class PreflightTests(unittest.TestCase):
    def test_real_structural_100case_preflight_binds_extra_sources_and_detects_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze,selection_sha,manifest_sha=synthetic_preflight_fixture(root)
            actual_verify=screen.common.verify_binding
            def verify_without_model(root,binding,*args):
                if binding["path"] in (screen.process.MODEL_PATH,screen.process.VAD_PATH):
                    return None
                return actual_verify(root,binding,*args)
            with mock.patch.multiple(screen.process,SELECTION_SHA=selection_sha,MANIFEST_SHA=manifest_sha), \
                 mock.patch.object(screen.common,"verify_binding",side_effect=verify_without_model):
                plan=screen.preflight(root/"freeze.json",root)
                self.assertEqual(sum(len(block["cases"]) for block in plan["blocks"]),400)
                self.assertFalse(plan["output"].exists())
                source=root/"src-tauri/src/runtime.rs"
                source.write_bytes(b"changed after preflight")
                with self.assertRaises(ValueError): screen.common.check_unchanged(plan)
                with self.assertRaises(ValueError): screen.preflight(root/"freeze.json",root)


if __name__ == "__main__":
    unittest.main()
