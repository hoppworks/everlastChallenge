"""Controlled report/process tests; no model, actual scheduling or qualification."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from test_full_prefix_screening import valid_reports

ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("full_prefix_remainder",ROOT/"scripts/run-full-prefix-remainder.py")
runner=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


def reports(root):
    original,_,case,paths,base=valid_reports(root)
    plan=dict(sourceRevision=original["sourceRevision"],model=original["model"],
        parameters=original["parameters"]["FullPrefixCandidate"],
        parameterSha256=original["parameterBindings"]["FullPrefixCandidate"]["sha256"])
    rows,results=[],[]
    for index in range(11):
        row=copy.deepcopy(base["runs"][0])
        row.update(id="warmup" if index==0 else "measured-%03d"%index,warmup=index==0,session=index+1)
        trace=row["jobTrace"]["trace"]
        trace["session"]=index+1
        for job,proof in zip(trace["jobs"],row["jobTrace"]["inputProofs"]):
            job["session"]=index+1
            job["jobId"]+=index*2
            job["preparedReuse"]["session"]=index+1
            job["preparedReuse"]["jobId"]+=index*2
            job["preparedReuse"]["originJobId"]+=index*2
            proof["jobId"]+=index*2
        rows.append(row)
        results.append(dict(id=row["id"],warmup=index==0,status="ok",text="controlled synthetic result"))
    base.update(measuredRunsRequested=10,engineTimingCoverage=dict(measuredRuns=10,withActualEngineTiming=10),
                runs=rows,jobTraceAfterCleanup=copy.deepcopy(rows[-1]["jobTrace"]["trace"]))
    paths["metadata"].write_text(json.dumps(base))
    paths["output"].write_text(json.dumps(results))
    return plan,case,paths,base,results


class ReportTests(unittest.TestCase):
    def test_eleven_actual_shaped_rows_use_overlap_unique_denominator(self):
        with tempfile.TemporaryDirectory() as directory:
            plan,case,paths,_,_=reports(Path(directory))
            with mock.patch.object(runner.process,"validate_trace",side_effect=AssertionError("disjoint validator called")):
                result=runner.validate_case(plan,case,paths)
            self.assertEqual(result["errors"],[])
            self.assertEqual(len(result["rows"]),11)
            self.assertTrue(all(r["validated"] and r["processedFramesSum"]==22400 and r["recordingFrames"]==16000 for r in result["rows"]))

    def test_wrong_modes_counts_future_audio_and_receipts_fail_closed(self):
        mutations=(
            lambda m:m.update(kind="paced-runtime-complete-recording"),
            lambda m:m.update(fullPrefixCandidate=False),
            lambda m:m.update(pretranscriptionCandidate=True),
            lambda m:m.update(measuredRunsRequested=1),
            lambda m:m["runs"].pop(),
            lambda m:m["runs"][10]["jobTrace"]["trace"]["jobs"][0].update(hostSubmitOffsetNanos=1),
            lambda m:m["runs"][10]["jobTrace"]["trace"]["jobs"][1]["preparedReuse"].update(originJobId=1),
            lambda m:m["runs"][10].update(engineInputFrames=16000),
            lambda m:m.update(jobTraceAfterCleanupError="controlled late failure"),
            lambda m:m["jobTraceAfterCleanup"]["jobs"][0]["preparedReuse"].update(nativeFullAttempted=1),
        )
        with tempfile.TemporaryDirectory() as directory:
            plan,case,paths,base,_=reports(Path(directory))
            for index,mutate in enumerate(mutations):
                value=copy.deepcopy(base)
                mutate(value)
                paths["metadata"].write_text(json.dumps(value))
                with self.subTest(mutation=index):self.assertTrue(runner.validate_case(plan,case,paths)["errors"])


def execution_plan(root):
    cases=[]
    for i in range(94):
        stem="artifacts/remainder/%03d"%i
        cases.append(dict(id="case-%03d"%i,expectedOutcome="Empty" if i<6 else "Local",
            output=stem+".json",metadata=stem+".json.metadata.json",log=stem+".log",completion=stem+".completion.json"))
    return dict(root=root,output=root/"artifacts/remainder",cases=cases,sourceRevision="a"*40,runId="synthetic-only")


class ExecutionTests(unittest.TestCase):
    def test_94_complete_cases_preserve_1034_slots_without_aggregate_qualification(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/"artifacts").mkdir()
            plan,calls=execution_plan(root),[]
            def launch(case,paths):
                calls.append(case["id"])
                for key in ("output","metadata","log"):runner.common.write_private(paths[key],{})
                return dict(exitCode=0,terminationConfirmed=True,launchError=None)
            def validate(_plan,case,_paths):
                return dict(errors=[],rows=runner.rows_for(case,"ok"),processId=10000+len(calls))
            with mock.patch.object(runner.process,"power_snapshot",return_value=dict(source="AC Power",error=None)), \
                 mock.patch.object(runner,"validate_case",side_effect=validate):
                result=runner.execute(plan,launch,lambda:True)
            self.assertEqual(len(calls),94)
            self.assertTrue(result["completed"])
            self.assertEqual(result["rowCount"],1034)
            self.assertIsNone(result["L"])
            self.assertFalse(result["qualified"])
            self.assertIsNone(result["statistics"])
            self.assertEqual(len(list(plan["output"].glob("*.completion.json"))),94)
            with self.assertRaises(FileExistsError):runner.execute(plan,launch)

    def test_first_nonzero_exit_preserves_partial_rows_and_stops_later_models(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/"artifacts").mkdir()
            plan,calls=execution_plan(root),[]
            def launch(case,paths):
                calls.append(case["id"])
                for key in ("output","metadata","log"):runner.common.write_private(paths[key],{})
                return dict(exitCode=3,terminationConfirmed=True,launchError=None)
            partial=runner.rows_for(plan["cases"][0],"missing")
            partial[0].update(status="ok",validated=True)
            with mock.patch.object(runner.process,"power_snapshot",return_value=dict(source="AC Power",error=None)), \
                 mock.patch.object(runner,"validate_case",return_value=dict(errors=["controlled fault"],rows=partial,processId=12001)):
                result=runner.execute(plan,launch,lambda:True)
            self.assertEqual(len(calls),1)
            self.assertFalse(result["completed"])
            self.assertEqual(result["cases"][0]["rows"][0]["status"],"ok")
            self.assertEqual([c["status"] for c in result["cases"]][1:],["unavailable"]*93)
            self.assertEqual(result["rowCount"],1034)

    def test_battery_before_start_blocks_directory_and_process(self):
        with tempfile.TemporaryDirectory() as directory:
            plan=execution_plan(Path(directory).resolve())
            launch=mock.Mock()
            with mock.patch.object(runner.process,"power_snapshot",return_value=dict(source="Battery Power",error=None)), \
                 self.assertRaises(runner.process.PowerAdmissionError):runner.execute(plan,launch)
            launch.assert_not_called()
            self.assertFalse(plan["output"].exists())


def preflight_fixture(root):
    """Real FD/Git/printer/WAV boundaries; model/decision authority substituted.

    This synthetic corpus is not the actual fixed94 or a passed quality report.
    """
    import os
    import subprocess
    import sys
    from test_full_prefix_screening import synthetic_preflight_fixture
    old,_,_=synthetic_preflight_fixture(root)
    def bind(name,data,mode=0o600):
        path=root/name
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data)
        path.chmod(mode)
        return dict(path=name,bytes=len(data),mode=oct(mode),sha256=runner.common.sha256(data))
    def document(name,value):return bind(name,json.dumps(value).encode())
    original=json.loads((root/old["manifest"]["path"]).read_bytes())
    manifest=document("rest-source.json",dict(cases=original["cases"][:94]))
    selected=json.loads((root/old["selection"]["path"]).read_bytes())["cases"][:94]
    for case in selected:case.update(sourceManifest=manifest["path"],warmupRuns=1,measuredRuns=10)
    central=document("rest-central.json",dict(cases=[]))
    selection=document("rest-selection.json",dict(cases=selected,sourceManifests=[manifest],centralManifest=central))
    params=json.loads((root/old["parameters"]["FullPrefixCandidate"]["path"]).read_bytes())
    params["models"]=[dict(id="large-v3-turbo-q5_0",sha256=runner.common.MODEL_SHA)]
    params["runtime"]["speechGate"]={"model":{"sha256":runner.common.VAD_SHA}}
    parameter=document("rest-params.json",params)
    # The executable accepts only the printer flag; no model path can execute.
    binary=bind(old["binary"]["path"],("#!"+sys.executable+"\nimport sys\n"
        "if sys.argv[1:]!=['--print-full-prefix-candidate-parameters']: raise SystemExit(3)\n"
        "sys.stdout.write("+repr(json.dumps(params))+")\n").encode(),0o755)
    sources={source["path"]:source for source in old["sources"]}
    for name in sorted(runner.OWN_SOURCES):
        data=(ROOT/name).read_bytes() if (ROOT/name).is_file() else b"# synthetic shared decision authority boundary\n"
        sources[name]=bind(name,data)
    for command in (["git","add",*sorted(sources)],
                    ["git","-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-q","-m","rest structural fixture"]):
        subprocess.run(command,cwd=root,check=True,capture_output=True)
    revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=root).decode().strip()
    commands=[]
    for i,case in enumerate(selected):
        stem="artifacts/rest/%03d"%i
        row=dict(id=case["id"],output=stem+".json",metadata=stem+".json.metadata.json",log=stem+".log",completion=stem+".completion.json")
        row["command"]=["/usr/bin/time","-l",str(root/binary["path"]),str(root/runner.process.MODEL_PATH),"reference",
            str(root/case["audio"]["path"]),case["language"],str(root/parameter["path"]),str(root/row["output"]),"10"]
        if case["expectedOutcome"]=="Empty":row["command"].append("--expect-empty")
        commands.append(row)
    model=dict(old["models"][0],id="large-v3-turbo-q5_0")
    freeze=dict(schemaVersion=1,kind=runner.KIND,runId="424273c0-04bc-4cdc-a30e-cfa2cec53d7a",sourceRevision=revision,
        binary=binary,parameters=parameter,selection=selection,sources=list(sources.values()),
        launcher=sources["scripts/run-full-prefix-remainder.py"],model=model,vad=old["models"][1],
        inputs=old["inputs"][:94],outputDirectory="artifacts/rest",publisherPolicy="timeConstrained",caseTimeoutSeconds=7200,
        qualityDecision=document("synthetic-decision.json",dict(controlledTestAuthority=True)),qualified=False,L=None,cases=commands)
    document("rest-freeze.json",freeze)
    return freeze


class PreflightTests(unittest.TestCase):
    def test_actual94_structural_preflight_keeps_sources_inputs_and_decision_fingerprinted(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze=preflight_fixture(root)
            actual=runner.common.verify_binding
            def without_model(root,binding,*args):
                if binding["path"] in (runner.process.MODEL_PATH,runner.process.VAD_PATH):return None
                return actual(root,binding,*args)
            def controlled_decision(root,binding,fp,_parameters):
                # Explicit external authority boundary, not an actual pass.
                return actual(root,binding,fp,runner.common.MAX_JSON,True)
            with mock.patch.object(runner.common,"SELECTION_SHA",freeze["selection"]["sha256"]), \
                 mock.patch.object(runner.common,"verify_binding",side_effect=without_model), \
                 mock.patch.object(runner,"validate_decision",side_effect=controlled_decision):
                plan=runner.preflight(root/"rest-freeze.json",root)
                self.assertEqual(len(plan["cases"]),94)
                self.assertEqual(sum(c["expectedOutcome"]=="Empty" for c in plan["cases"]),6)
                self.assertEqual(len([p for p in plan["fingerprints"] if p.suffix==".wav"]),94)
                self.assertIn(root/"synthetic-decision.json",plan["fingerprints"])
                self.assertFalse(plan["output"].exists())
                (root/"synthetic-decision.json").write_bytes(b"changed after preflight")
                with self.assertRaises(ValueError):runner.common.check_unchanged(plan)

    def test_nonadmitted_quality_or_nonexact_command_cannot_become_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze=preflight_fixture(root)
            actual=runner.common.verify_binding
            def without_model(root,binding,*args):
                if binding["path"] in (runner.process.MODEL_PATH,runner.process.VAD_PATH):return None
                return actual(root,binding,*args)
            with mock.patch.object(runner.common,"SELECTION_SHA",freeze["selection"]["sha256"]), \
                 mock.patch.object(runner.common,"verify_binding",side_effect=without_model):
                with mock.patch.object(runner,"validate_decision",side_effect=ValueError("no actual passing decision")), \
                     self.assertRaises(ValueError):runner.preflight(root/"rest-freeze.json",root)
                freeze["cases"][0]["command"][-2]="30" # Empty final flag follows count
                (root/"rest-freeze.json").write_text(json.dumps(freeze))
                with mock.patch.object(runner,"validate_decision",return_value=None), \
                     self.assertRaises(ValueError):runner.preflight(root/"rest-freeze.json",root)
            self.assertFalse((root/"artifacts/rest").exists())


class OwnedBoundaryTests(unittest.TestCase):
    def test_actual_owned_exit_three_keeps_94_completions_and_private_raw_log(self):
        import sys
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/"artifacts").mkdir()
            plan=execution_plan(root)
            plan["cases"][0]["command"]=[sys.executable,"-c","raise SystemExit(3)"]
            with mock.patch.object(runner.process,"power_snapshot",return_value=dict(source="AC Power",error=None)):
                result=runner.execute(plan,lambda case,paths:runner.process.launch_case(plan,case,paths),lambda:True)
            self.assertFalse(result["completed"])
            first=result["cases"][0]
            self.assertEqual(first["launch"]["exitCode"],3)
            self.assertTrue(first["launch"]["terminationConfirmed"])
            self.assertEqual(len(result["cases"]),94)
            self.assertEqual(result["rowCount"],1034)
            self.assertEqual(len(list(plan["output"].glob("*.completion.json"))),94)
            self.assertTrue(all((p.stat().st_mode&0o777)==0o600 for p in plan["output"].iterdir()))

    def test_postcase_battery_invalidates_case_keeps_rows_and_stops_remaining(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/"artifacts").mkdir()
            plan,calls=execution_plan(root),[]
            def launch(case,paths):
                calls.append(case["id"])
                for key in ("output","metadata","log"):runner.common.write_private(paths[key],{})
                return dict(exitCode=0,terminationConfirmed=True,launchError=None)
            ac=dict(source="AC Power",error=None)
            battery=dict(source="Battery Power",error=None)
            validation=dict(errors=[],rows=runner.rows_for(plan["cases"][0],"ok"),processId=11001)
            with mock.patch.object(runner.process,"power_snapshot",side_effect=[ac,ac,battery]), \
                 mock.patch.object(runner,"validate_case",return_value=validation):
                result=runner.execute(plan,launch,lambda:True)
            self.assertEqual(len(calls),1)
            self.assertFalse(result["completed"])
            self.assertEqual(result["cases"][0]["status"],"failed")
            self.assertEqual(len(result["cases"][0]["rawFiles"]),3)
            self.assertEqual([r["status"] for r in result["cases"][0]["rows"]],["ok"]*11)
            self.assertEqual([c["status"] for c in result["cases"]][1:],["unavailable"]*93)
