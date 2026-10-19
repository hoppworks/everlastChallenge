import copy
import importlib.util
from pathlib import Path
import unittest
import json
import tempfile
import sys
import subprocess
from unittest import mock

from test_full_prefix_screening import trace_fixture, valid_reports, parameter_fixture, block_fixture, execution_plan, synthetic_preflight_fixture

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("context1400_screen", ROOT / "scripts/run-context1400-screening.py")
screen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screen)


class ContextReceiptTests(unittest.TestCase):
    def test_missing_per_job_context_is_rejected_even_when_old_trace_passes(self):
        evidence,row=trace_fixture()
        with self.assertRaisesRegex(ValueError,"context"):
            screen.validate_trace(evidence,row,16000,"de",True)


def add_contexts(evidence):
    for job in evidence["trace"]["jobs"]:
        receipt=job["preparedReuse"]
        short=receipt["preparedFrames"] is not None and receipt["preparedFrames"]<=320000
        job["contextReceipt"]=dict(session=job["session"],generation=job["generation"],jobId=job["jobId"],
            selection=None if receipt["outcome"]=="not-admitted" else dict(policy="short-context1400",
                reason="short-prepared" if short else "long-prepared-reference",parameter=1400 if short else 0,
                effectiveContext=1400 if short else 1500,modelContext=1500,preparedFrames=receipt["preparedFrames"]),
            nativeFullAttempted=receipt["nativeFullAttempted"],cacheOriginJobId=receipt["originJobId"])


class ContextBoundaryTests(unittest.TestCase):
    def test_hit_and_decoded_contexts_follow_real_prepared_identity(self):
        evidence,row=trace_fixture();add_contexts(evidence)
        self.assertEqual(screen.validate_trace(evidence,row,16000,"de",True)["jobCount"],2)
        for mutate in (
            lambda j:j["contextReceipt"].update(jobId=999),
            lambda j:j["contextReceipt"]["selection"].update(parameter=0),
            lambda j:j["contextReceipt"]["selection"].update(modelContext=1400),
            lambda j:j["contextReceipt"].update(nativeFullAttempted=1),
            lambda j:j["contextReceipt"].update(cacheOriginJobId=12),
            lambda j:j["contextReceipt"]["selection"].update(preparedFrames=15999),
        ):
            changed=copy.deepcopy(evidence);mutate(changed["trace"]["jobs"][-1])
            with self.subTest(mutate=mutate),self.assertRaises(ValueError):
                screen.validate_trace(changed,dict(row,jobTrace=changed),16000,"de",True)

    def test_320000_and_320001_use_prepared_not_recorded_frames(self):
        for prepared,parameter in ((320000,1400),(320001,0)):
            evidence,row=trace_fixture();job=evidence["trace"]["jobs"][-1]
            job.update(timelineEnd=400000,storageEnd=400000,inputFrames=400000,requiredStableFrames=400000)
            job["preparedReuse"].update(outcome="decoded",originJobId=job["jobId"],preparedFrames=prepared,
                decoderInvoked=True,nativeFullAttempted=True)
            evidence["trace"]["jobs"]=[job];evidence["inputProofs"]=[dict(jobId=job["jobId"],requiredStableFrames=400000,publishedThroughFramesAtAdmission=400000)]
            evidence.update(processedFramesSum=400000,checkedWorkerSpanSumNanos=99998)
            row.update(engineInputFrames=400000,engineElapsedNanos=10000,engineRtf=.0000004,sessionRtfRecordingFrames=400000,
                feed=dict(publications=[dict(startFrame=0,endFrame=400000,afterPublishNanos=1)]))
            add_contexts(evidence)
            self.assertEqual(job["contextReceipt"]["selection"]["parameter"],parameter)
            self.assertEqual(screen.validate_trace(evidence,row,400000,"de",False)["jobCount"],1)
            job["contextReceipt"]["selection"]["parameter"]=0 if parameter==1400 else 1400
            with self.assertRaises(ValueError):screen.validate_trace(evidence,row,400000,"de",False)

    def test_not_admitted_has_no_selected_or_native_context(self):
        evidence,row=trace_fixture();job=evidence["trace"]["jobs"][-1]
        evidence["trace"]["jobs"]=[job];evidence["inputProofs"]=evidence["inputProofs"][-1:]
        evidence.update(processedFramesSum=16000,checkedWorkerSpanSumNanos=99998)
        row.update(engineInputFrames=16000,engineElapsedNanos=10000,engineRtf=.00001)
        job["preparedReuse"].update(outcome="not-admitted",originJobId=None,preparedFrames=None,preparedPcmSha256=None,
            decoderInvoked=False,nativeFullAttempted=False)
        add_contexts(evidence)
        screen.validate_trace(evidence,row,16000,"de",False)
        job["contextReceipt"]["selection"]={"parameter":1400}
        with self.assertRaises(ValueError):screen.validate_trace(evidence,row,16000,"de",False)


class CaseAndPlanTests(unittest.TestCase):
    def test_actual_report_shape_retains_bad_context_and_requires_postjoin(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();plan,block,case,paths,meta=valid_reports(root)
            variant="p06-short-context1400-full-prefix-v1";block["variant"]=variant
            plan["parameters"]={"synthetic":True};plan["parameterBinding"]={"sha256":"c"*64};meta["parameters"]=plan["parameters"]
            for row in meta["runs"]:add_contexts(row["jobTrace"])
            meta["jobTraceAfterCleanup"]=copy.deepcopy(meta["runs"][-1]["jobTrace"]["trace"])
            paths["metadata"].write_text(json.dumps(meta))
            self.assertEqual(screen.validate_case(plan,block,case,paths)["errors"],[])
            meta["runs"][-1]["jobTrace"]["trace"]["jobs"][-1]["contextReceipt"]["nativeFullAttempted"]=True
            paths["metadata"].write_text(json.dumps(meta))
            result=screen.validate_case(plan,block,case,paths)
            self.assertTrue(result["errors"])
            self.assertEqual(json.loads(paths["output"].read_bytes())[1]["text"],"controlled public fixture")
            self.assertEqual(result["rows"][1]["status"],"ok")

    def test_only_two_full_same_variant_blocks_and_fresh_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();freeze,cases=block_fixture(root);variant="p06-short-context1400-single-v1"
            freeze["variant"]=variant;freeze["parameters"]={"path":"params.json"};freeze["blocks"]=freeze["blocks"][:2]
            for i,block in enumerate(freeze["blocks"]):
                block.update(name="C%d"%(i+1),variant=variant)
                for entry in block["cases"]:entry["command"][7]=str(root/"params.json")
            screen.validate_blocks(freeze,root,cases)
            freeze["blocks"][1]["variant"]="p06-short-context1400-full-prefix-v1"
            with self.assertRaises(ValueError):screen.validate_blocks(freeze,root,cases)
            freeze["blocks"][1]["variant"]=variant
            freeze["blocks"][1]["cases"].pop()
            with self.assertRaises(ValueError):screen.validate_blocks(freeze,root,cases)

    def test_first_failure_preserves_all400_rows_without_following_launches(self):
        with tempfile.TemporaryDirectory() as directory:
            plan=execution_plan(Path(directory).resolve());plan["variant"]="p06-short-context1400-single-v1";plan["blocks"]=plan["blocks"][:2]
            for i,b in enumerate(plan["blocks"]):b.update(name="C%d"%(i+1),variant=plan["variant"])
            calls=[]
            def launch(case,paths):calls.append(case["id"]);return dict(exitCode=3,terminationConfirmed=True,launchError=None)
            with mock.patch.object(screen.process,"power_snapshot",return_value={"source":"AC Power","error":None}):
                report=screen.execute(plan,launch)
            self.assertEqual(calls,["case-000"]);self.assertEqual(report["rowCount"],400);self.assertFalse(report["completed"])
            self.assertFalse(report["newABBA"]);self.assertFalse(report["historicalTimingReused"])
            self.assertEqual(sum(c["status"]=="unavailable" for b in report["blocks"] for c in b["cases"]),199)
            self.assertEqual(len(list(plan["output"].glob("*.completion.json"))),200)
            with self.assertRaises(FileExistsError):screen.execute(plan,launch)

    def test_battery_prevents_creation_and_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            plan=execution_plan(Path(directory).resolve());plan["blocks"]=plan["blocks"][:2];plan["variant"]="p06-short-context1400-single-v1"
            for i,b in enumerate(plan["blocks"]):b.update(name="C%d"%(i+1),variant=plan["variant"])
            with mock.patch.object(screen.process,"power_snapshot",return_value={"source":"Battery Power","error":None}),self.assertRaises(screen.process.PowerAdmissionError):
                screen.execute(plan,mock.Mock(side_effect=AssertionError("model launch forbidden")))
            self.assertFalse(plan["output"].exists())


class ParameterAndAuthorityTests(unittest.TestCase):
    def test_exact_selection_and_other_decoder_values_cannot_change(self):
        reference=parameter_fixture(False)
        reference.update(state={"fixed":True},preprocessing={"fixed":True},models=["original"])
        for variant,full_prefix in screen.VARIANTS.items():
            candidate=copy.deepcopy(reference);candidate["configurationId"]=variant;candidate["contextPolicy"]=screen.CONTEXT_POLICY.copy()
            candidate["runtime"]["fullParams"]["audio_ctx"]={"selectedPerPreparedInput":"contextPolicy"}
            candidate["fullPrefix"].update(candidate=full_prefix,
                decoderPolicy="fixed ShortContext1400; prepared1..320000 selects1400, longer selects verified model default0; otherwise unchanged",
                cache="exact prepared f32LE SHA256+frames+language+immutable ContextPolicy+selected parameter; current validation/energy/VAD/padding before every lookup")
            screen.validate_parameters(candidate,variant,reference)
            for mutate in (lambda p:p["contextPolicy"].update(prepared1Through320000=1399),
                           lambda p:p["runtime"]["fullParams"].update(no_timestamps=True),
                           lambda p:p.update(configurationId="p06-full-prefix-reference-v1"),
                           lambda p:p["fullPrefix"].update(candidate=not full_prefix)):
                changed=copy.deepcopy(candidate);mutate(changed)
                with self.assertRaises(ValueError):screen.validate_parameters(changed,variant,reference)

    def test_no_self_approval_or_foreign_reference_equivalence(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();evidence=root/"evidence.json";evidence.write_bytes(b"external source review")
            eb=screen.common.small_binding(evidence);eb["path"]="evidence.json"
            freeze=dict(sourceRevision="b"*40,variant="p06-short-context1400-single-v1",
                historicalReferenceApproval={"sha256":"a"*64},parameters={"sha256":"c"*64},manifest={"sha256":"d"*64})
            history={"source_revision":"301fee4cea4d9c87023bf949838d3468ecba2e95"}
            authority=dict(schemaVersion=1,kind="context1400-reference-equivalence-v1",passed=True,
                scope="historical-reference-text-comparison-only",historicalReferenceApprovalSha256="a"*64,
                historicalSourceRevision=history["source_revision"],candidateSourceRevision="b"*40,
                candidateVariant=freeze["variant"],candidateParametersSha256="c"*64,manifestSha256="d"*64,
                timingTransfer=False,productionApproval=False,reviewEvidence=eb)
            path=root/"authority.json"
            def call(value):
                path.write_text(json.dumps(value));binding=screen.common.small_binding(path);binding["path"]="authority.json"
                return screen.validate_equivalence(root,binding,{},freeze,history)
            call(authority)
            for key,value in (("passed",False),("historicalSourceRevision","e"*40),("candidateParametersSha256","f"*64),
                              ("timingTransfer",True),("productionApproval",True)):
                changed=dict(authority);changed[key]=value
                with self.assertRaises(ValueError):call(changed)
            with self.assertRaises(ValueError):screen.validate_equivalence(root,None,{},freeze,history)


class PreflightAdmissionTests(unittest.TestCase):
    def test_missing_external_equivalence_blocks_before_any_binary_or_model_access(self):
        from test_historical_reference import HistoricalReferenceTests
        fixture=HistoricalReferenceTests(methodName="test_original_four_arrays_are_bytes_and_never_new_run_authority")
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        root=fixture.root
        authority=fixture.authority()
        historical=screen.historical_reference.load(root,authority)
        reference=json.loads((root/historical["provenance"]["parameters"]["FullPrefixReference"]["path"]).read_bytes())
        variant="p06-short-context1400-single-v1"
        document=copy.deepcopy(reference)
        document.update(configurationId=variant,contextPolicy=screen.CONTEXT_POLICY.copy(),build=dict(debugAssertions=False),
                        preprocessing={},state={},models=[],measurement=dict(mode="paced-full-prefix-revision-v1",chunkMillis=20,
                        jitterToleranceNanos=10000000,maximumSleepNanos=1000000,finalSpinNanos=250000,publisherThread=dict(policy="timeConstrained")))
        reference.update(preprocessing={},state={},models=[])
        # Replace only synthetic old-parameter metadata, then rebuild its trusted synthetic authority.
        fixture.params["FullPrefixReference"].update(preprocessing={},state={},models=[])
        fixture.params["FullPrefixCandidate"].update(preprocessing={},state={},models=[])
        authority=fixture.authority()
        document["runtime"]["fullParams"]={"audio_ctx":{"selectedPerPreparedInput":"contextPolicy"}}
        fixture.runtime["fullParams"]={"audio_ctx":0}
        for param in fixture.params.values():param["runtime"]=copy.deepcopy(fixture.runtime)
        authority=fixture.authority()
        document["runtime"]=copy.deepcopy(fixture.runtime)
        document["runtime"]["fullParams"]["audio_ctx"]={"selectedPerPreparedInput":"contextPolicy"}
        document["fullPrefix"].update(decoderPolicy="fixed ShortContext1400; prepared1..320000 selects1400, longer selects verified model default0; otherwise unchanged",
            cache="exact prepared f32LE SHA256+frames+language+immutable ContextPolicy+selected parameter; current validation/energy/VAD/padding before every lookup")
        selection_bytes=(ROOT/"tests/fixtures/corpus/p06-context-quality-selection-v1.json").read_bytes()
        selection=fixture.write("selection.json",selection_bytes)
        manifest=fixture.write("manifest.json",fixture.manifest_bytes)
        params=fixture.write("candidate.json",document)
        freeze=dict(schemaVersion=1,kind=screen.KIND,screeningId="424273c0-04bc-4cdc-a30e-cfa2cec53d7a",sourceRevision="b"*40,
            binary={"path":"not-even-present/paced_runtime_probe"},selection=selection,manifest=manifest,parameters=params,sources=[],launcher={},models=[],inputs=[],
            outputDirectory="artifacts/new",publisherPolicy="timeConstrained",caseTimeoutSeconds=screen.process.WATCHDOG_SECONDS,
            qualified=False,L=None,blocks=[],variant=variant,historicalReferenceApproval=authority,referenceEquivalenceAuthority=None)
        binding=fixture.write("context-freeze.json",freeze)
        with mock.patch.object(screen.__dict__["full"].process,"immutable_source",side_effect=AssertionError("source check prematurely reached")), \
             mock.patch("subprocess.run",side_effect=AssertionError("binary must not run")), \
             self.assertRaisesRegex(ValueError,"reference-equivalence"):
            screen.preflight(root/binding["path"],root)
        self.assertFalse((root/"artifacts/new").exists())

    def test_real_files_git_and_print_only_preflight_with_explicit_authority_stubs(self):
        # No model/ASR. The separately tested historical loader is stubbed; model
        # bytes and fixed-corpus identity are replaced by this structural fixture.
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze,selection_sha,manifest_sha=synthetic_preflight_fixture(root)
            def write(path,data,mode=0o600):
                p=root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);p.chmod(mode)
                return dict(path=path,bytes=len(data),sha256=screen.common.sha256(data),mode=oct(mode))
            def document(path,value):return write(path,json.dumps(value).encode())
            for path in screen.OWN_SOURCES:write(path,(ROOT/path).read_bytes())
            context=write("src-tauri/src/transcription/context_policy.rs",b"// synthetic context policy marker\n")
            reference=parameter_fixture(False);reference.update(preprocessing={},state={},models=[])
            core_paths={"src-tauri/src/runtime.rs"}|{"synthetic-core/%02d.rs"%i for i in range(46)}
            reference["sourceHashes"]={}
            for index,path in enumerate(sorted(core_paths)):
                core=write(path,("// synthetic bound core %d\n"%index).encode())
                reference["sourceHashes"]["core%02d"%index]=dict(filename=path,sha256=core["sha256"])
            oldparam=document("old-reference.json",reference)
            variant="p06-short-context1400-full-prefix-v1"
            parameter=copy.deepcopy(reference);parameter.update(configurationId=variant,contextPolicy=screen.CONTEXT_POLICY.copy())
            parameter["runtime"]["fullParams"]["audio_ctx"]={"selectedPerPreparedInput":"contextPolicy"}
            parameter["fullPrefix"].update(candidate=True,
                decoderPolicy="fixed ShortContext1400; prepared1..320000 selects1400, longer selects verified model default0; otherwise unchanged",
                cache="exact prepared f32LE SHA256+frames+language+immutable ContextPolicy+selected parameter; current validation/energy/VAD/padding before every lookup")
            parameter["sourceHashes"]["contextPolicy"]=dict(filename=context["path"],sha256=context["sha256"])
            parameter_binding=document("candidate.json",parameter)
            printer=("#!"+sys.executable+"\nimport sys\n"
                     "if sys.argv[1:]!=['--print-short-context1400-full-prefix-parameters']:raise SystemExit(9)\n"
                     "print("+repr(json.dumps(parameter))+")\n").encode()
            freeze["binary"]=write(freeze["binary"]["path"],printer,0o755)
            freeze["parameters"]=parameter_binding;freeze["kind"]=screen.KIND;freeze["variant"]=variant
            freeze["blocks"]=freeze["blocks"][:2]
            for i,block in enumerate(freeze["blocks"]):
                block.update(name="C%d"%(i+1),variant=variant)
                for case in block["cases"]:case["command"][7]=str(root/parameter_binding["path"])
            freeze["historicalReferenceApproval"]=document("old-approval.json",{"externalSyntheticAuthority":True})
            review=document("equivalence-review.json",{"externalSyntheticSourceReview":True})
            paths=set(screen.OWN_SOURCES)|core_paths|{context["path"]}
            freeze["sources"]=[dict(path=p,**{k:v for k,v in screen.common.small_binding(root/p).items() if k!='path'}) for p in sorted(paths)]
            subprocess.run(["git","add",*sorted(paths)],cwd=root,check=True,capture_output=True)
            subprocess.run(["git","-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-qm","new structural source"],cwd=root,check=True,capture_output=True)
            freeze["sourceRevision"]=subprocess.check_output(["git","rev-parse","HEAD"],cwd=root,text=True).strip()
            freeze["launcher"]=next(b for b in freeze["sources"] if b["path"]=="scripts/run-context1400-screening.py")
            freeze["referenceEquivalenceAuthority"]=document("equivalence.json",dict(schemaVersion=1,kind="context1400-reference-equivalence-v1",passed=True,
                scope="historical-reference-text-comparison-only",historicalReferenceApprovalSha256=freeze["historicalReferenceApproval"]["sha256"],
                historicalSourceRevision=screen.historical_reference.SOURCE_REVISION,candidateSourceRevision=freeze["sourceRevision"],candidateVariant=variant,
                candidateParametersSha256=parameter_binding["sha256"],manifestSha256=manifest_sha,timingTransfer=False,productionApproval=False,reviewEvidence=review))
            document("freeze.json",freeze)
            history=dict(source_revision=screen.historical_reference.SOURCE_REVISION,manifest_bytes=(root/freeze["manifest"]["path"]).read_bytes(),
                provenance=dict(parameters={"FullPrefixReference":oldparam}))
            actual_verify=screen.common.verify_binding
            def no_models(root,binding,*args):
                if binding["path"] in (screen.process.MODEL_PATH,screen.process.VAD_PATH):return None
                return actual_verify(root,binding,*args)
            with mock.patch.multiple(screen.process,SELECTION_SHA=selection_sha,MANIFEST_SHA=manifest_sha), \
                 mock.patch.object(screen.historical_reference,"load",return_value=history), \
                 mock.patch.object(screen.common,"verify_binding",side_effect=no_models), \
                 mock.patch.object(screen,"__file__",str(root/"scripts/run-context1400-screening.py")), \
                 mock.patch.object(screen.full,"__file__",str(root/"scripts/run-full-prefix-screening.py")), \
                 mock.patch.object(screen.process,"__file__",str(root/"scripts/run-pretranscription-screening.py")), \
                 mock.patch.object(screen.common,"__file__",str(root/"scripts/run-paced-remainder.py")), \
                 mock.patch.object(screen.historical_reference,"__file__",str(root/"scripts/screening_quality/historical_reference.py")), \
                 mock.patch.object(screen.historical_reference.decision,"__file__",str(root/"scripts/screening_quality/decision.py")):
                plan=screen.preflight(root/"freeze.json",root)
                self.assertEqual(sum(len(b["cases"]) for b in plan["blocks"]),200)
                self.assertFalse(plan["output"].exists())
                self.assertEqual(plan["historicalReference"]["sourceRevision"],screen.historical_reference.SOURCE_REVISION)
                self.assertEqual(len(screen.common.compiled_sources(plan["parameters"])),48)
                saved={name:(root/name).read_bytes() for name in ("candidate.json",freeze["binary"]["path"],"equivalence.json","freeze.json")}
                changed=copy.deepcopy(parameter)
                missing=next(k for k,v in changed["sourceHashes"].items() if v["filename"]=="src-tauri/src/runtime.rs")
                del changed["sourceHashes"][missing]
                freeze["parameters"]=document("candidate.json",changed)
                changed_printer=("#!"+sys.executable+"\nimport sys\n"
                    "if sys.argv[1:]!=['--print-short-context1400-full-prefix-parameters']:raise SystemExit(9)\n"
                    "print("+repr(json.dumps(changed))+")\n").encode()
                freeze["binary"]=write(freeze["binary"]["path"],changed_printer,0o755)
                equivalence=json.loads((root/"equivalence.json").read_bytes())
                equivalence["candidateParametersSha256"]=freeze["parameters"]["sha256"]
                freeze["referenceEquivalenceAuthority"]=document("equivalence.json",equivalence)
                document("freeze.json",freeze)
                with self.assertRaisesRegex(ValueError,"48.*source closure"):
                    screen.preflight(root/"freeze.json",root)
                for name,data in saved.items():(root/name).write_bytes(data)
                (root/context["path"]).write_bytes(b"changed")
                with self.assertRaises(ValueError):screen.common.check_unchanged(plan)
                with self.assertRaises(ValueError):screen.preflight(root/"freeze.json",root)


if __name__ == "__main__":
    unittest.main()
