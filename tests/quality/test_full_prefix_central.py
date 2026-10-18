import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from test_pretranscription_central import cases

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("shared_central_supervisor",ROOT / "scripts/run-pretranscription-central.py")
central = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(central)


class PairSupervisorTests(unittest.TestCase):
    def test_explicit_full_prefix_validator_uses_existing_owned_pair_and_statistics(self):
        with tempfile.TemporaryDirectory() as name:
            plan = dict(outputRoot=Path(name),cases=cases(),freezeBytes=b"{}",freezeSha256="f"*64)
            calls = []

            def launch(plan,variant,paths):
                calls.append(variant)
                for key,value in (("results",[]),("metadata",{})):
                    central.common.write_private(paths[key],value)
                central.common.write_private_bytes(paths["log"],b"")
                return dict(exitCode=0,terminationConfirmed=True,launchError=None)

            def validate(plan,variant,*unused):
                return dict(processId=100+len(calls),rows=[dict(slot=s,status="ok",validated=True)
                    for s in central.slots(plan["cases"])],statistics=central.statistics(plan["cases"],
                    [1_000_000_000 if variant=="reference" else 800_000_000]*180))

            with mock.patch.object(central,"validate_documents",side_effect=AssertionError("legacy disjoint validation forbidden")):
                report = central.execute_pair(plan,"cand-ref",launch,validator=validate,
                    kind="full-prefix-central-pair-execution-v1")
            self.assertEqual(calls,["candidate","reference"])
            self.assertEqual(report["kind"],"full-prefix-central-pair-execution-v1")
            self.assertTrue(report["completed"])
            self.assertTrue(report["comparison"]["relativeImprovementPassed"])
            self.assertEqual([len(b["rows"]) for b in report["blocks"]],[181,181])
            self.assertFalse((Path(name)/".active-pair").exists())


if __name__ == "__main__":
    unittest.main()


FULL_SPEC=importlib.util.spec_from_file_location("full_prefix_central_adapter",ROOT/"scripts/run-full-prefix-central.py")
full=importlib.util.module_from_spec(FULL_SPEC)
FULL_SPEC.loader.exec_module(full)


def full_parameters(candidate):
    from test_full_prefix_screening import parameter_fixture
    value=parameter_fixture(candidate)
    value["configurationId"]="p06-six-case-full-prefix-"+("candidate" if candidate else "reference")+"-v1"
    value["preprocessing"]={"mode":("continuous stable DSP; cumulative full-prefix revisions and complete final recording; current normal energy/VAD/preparation before every lookup" if candidate else
        "matched full recording only; same scoped cache decoder and normal energy/VAD/preparation; fresh scope cannot reuse prior session")}
    value["measurement"].update(warmupRuns=1,measuredRunsPerCase=30,totalRows=181,
        variant="full-prefix-revisions" if candidate else "matched-full-recording-reference")
    return value


def full_documents(candidate=False):
    import copy
    from test_pretranscription_central import documents
    plan,results,meta=documents(candidate)
    plan["parameters"]={"reference":full_parameters(False),"candidate":full_parameters(True)}
    meta.update(kind="paced-six-case-full-prefix-experiment",hostObservation=dict(powerSource=dict(value="AC Power",error=None)),
        pretranscriptionCandidate=None,fullPrefixCandidate=candidate,
        parameters=plan["parameters"]["candidate" if candidate else "reference"])
    for i,record in enumerate(meta["runs"]):
        row=record["observation"]
        evidence=row["jobTrace"]
        final=evidence["trace"]["jobs"][0]
        final["jobId"]=2*i+2
        final["completionObservedOffsetNanos"]=final["resultAdoptedOffsetNanos"]-1
        final["preparedReuse"]=dict(session=i+1,generation=1,jobId=final["jobId"],outcome="decoded",
            originJobId=final["jobId"],preparedFrames=final["inputFrames"],preparedPcmSha256=[17]*32,
            decoderInvoked=True,nativeFullAttempted=True)
        proof=evidence["inputProofs"][0]
        proof.pop("lastCompletePublicationOffsetNanos")
        proof["jobId"]=final["jobId"]
        evidence.update(processedFramesSum=final["inputFrames"],checkedWorkerSpanSumNanos=100_000_000,
            finalAdoptionOffsetNanos=final["resultAdoptedOffsetNanos"])
        if candidate:
            early=copy.deepcopy(final)
            cut=final["inputFrames"]//2
            start=(cut+4800)*62500+20
            early.update(jobId=2*i+1,timelineEnd=cut,storageEnd=cut,inputFrames=cut,audioReleasedAtSubmit=False,
                requiredStableFrames=cut+4800,hostSubmitOffsetNanos=start,serviceAcceptedOffsetNanos=start+10,
                workerBeforeDecodeOffsetNanos=start+20,workerAfterDecodeOffsetNanos=start+100_000_020,
                workerSettledOffsetNanos=start+100_000_030,completionObservedOffsetNanos=start+100_000_039,
                resultAdoptedOffsetNanos=start+100_000_040)
            early["preparedReuse"].update(jobId=early["jobId"],originJobId=early["jobId"],preparedFrames=cut)
            evidence["trace"]["jobs"].insert(0,early)
            evidence["inputProofs"].insert(0,dict(jobId=early["jobId"],requiredStableFrames=cut+4800,
                publishedThroughFramesAtAdmission=cut+4800))
            evidence["processedFramesSum"]+=cut
            evidence["checkedWorkerSpanSumNanos"]+=100_000_000
            row["engineElapsedNanos"]=200_000_000
        row["sessionRtfRecordingFrames"]=final["inputFrames"]
        row["engineInputFrames"]=evidence["processedFramesSum"]
        row["engineRtf"]=row["engineElapsedNanos"]*16000/final["inputFrames"]/1e9
    meta["jobTraceAfterCleanup"]=copy.deepcopy(meta["runs"][-1]["observation"]["jobTrace"]["trace"])
    meta["statistics"]=central.statistics(plan["cases"],[1_000_000_000]*180)
    from test_pretranscription_central import results_sha
    meta["hypothesesSha256"]=results_sha(results)
    return plan,results,meta


class FullDocumentsTests(unittest.TestCase):
    def test_exact_batch_parameter_pair_and_181_overlap_rows_both_arms(self):
        from test_pretranscription_central import results_sha
        for candidate in (False,True):
            plan,results,meta=full_documents(candidate)
            full.validate_parameters(plan["parameters"])
            checked=full.validate_documents(plan,"candidate" if candidate else "reference",results,meta,results_sha(results))
            self.assertEqual(len(checked["rows"]),181)
            self.assertEqual(checked["statistics"]["lNanos"],1_000_000_000)
            self.assertEqual(checked["rows"][0]["processedFramesSum"],120000 if candidate else 80000)

    def test_old_mode_and_forged_full_prefix_trace_stats_or_hash_are_rejected(self):
        import copy
        from test_pretranscription_central import results_sha
        plan,results,base=full_documents(True)
        mutations=(lambda m:m.update(kind="paced-six-case-pretranscription-experiment"),
            lambda m:m.update(pretranscriptionCandidate=True),lambda m:m.update(fullPrefixCandidate=False),
            lambda m:m["runs"][0]["observation"]["jobTrace"]["trace"]["jobs"][0].update(requiredStableFrames=0),
            lambda m:m["runs"][10]["observation"]["jobTrace"]["trace"]["jobs"][0]["preparedReuse"].update(session=1),
            lambda m:m["runs"][10]["observation"].update(engineInputFrames=80000),
            lambda m:m.update(jobTraceAfterCleanup={}),lambda m:m["statistics"].update(lNanos=1),
            lambda m:m.update(hypothesesSha256="0"*64))
        for i,mutate in enumerate(mutations):
            value=copy.deepcopy(base)
            mutate(value)
            with self.subTest(mutation=i),self.assertRaises(ValueError):
                full.validate_documents(plan,"candidate",results,value,results_sha(results))


def preflight_fixture(root):
    """Synthetic FD/Git/PCM/printer fixture, never real quality/model evidence."""
    import io
    import json
    import subprocess
    import sys
    import wave
    def bind(name,data,mode=0o600):
        p=root/name
        p.parent.mkdir(parents=True,exist_ok=True)
        p.write_bytes(data)
        p.chmod(mode)
        return dict(path=name,bytes=len(data),sha256=full.common.sha256(data),mode=oct(mode))
    def document(name,value):return bind(name,json.dumps(value).encode())
    compiled=bind("source.rs",b"// synthetic compiled boundary\n")
    sources=[compiled]+[bind(name,(ROOT/name).read_bytes()) for name in sorted(full.OWN)]
    ps={v:full_parameters(v=="candidate") for v in full.IDS}
    for value in ps.values():
        value["sourceHashes"]={"controlled":dict(filename=compiled["path"],sha256=compiled["sha256"])}
        value["runtime"]["speechGate"]={"model":{"sha256":full.common.VAD_SHA}}
        value["models"]=[dict(id="large-v3-turbo-q5_0",sha256=full.common.MODEL_SHA)]
    parameters={v:document(v+".json",p) for v,p in ps.items()}
    printed={"--print-full-prefix-"+v+"-parameters":json.dumps(p) for v,p in ps.items()}
    binary=bind("target/release/examples/paced_reference_probe",("#!"+sys.executable+"\nimport sys\nvalues="+repr(printed)+"\n"
        "if len(sys.argv)!=2 or sys.argv[1] not in values: raise SystemExit(3)\n"
        "sys.stdout.write(values[sys.argv[1]])\n").encode(),0o755)
    cs,inputs=cases(),[]
    for i,c in enumerate(cs):
        stream=io.BytesIO()
        with wave.open(stream,"wb") as wav:
            wav.setparams((1,2,16000,c["audio"]["frames"],"NONE","not compressed"))
            wav.writeframes(bytes([i,0])*c["audio"]["frames"])
        inputs.append(bind(c["audio"]["path"],stream.getvalue()))
        c["audio"]["sha256"]=inputs[-1]["sha256"]
    manifest=document(full.CENTRAL_MANIFEST_PATH,dict(schemaVersion=1,unicodeVersion="13.0.0",cases=cs))
    model=dict(id="large-v3-turbo-q5_0",path="artifacts/models/ggml-large-v3-turbo-q5_0.bin",
        bytes=574041195,sha256=full.common.MODEL_SHA,mode="0o600")
    vad=dict(path="artifacts/models/ggml-silero-v5.1.2.bin",bytes=885098,sha256=full.common.VAD_SHA,mode="0o600")
    for args in (["git","init","-q"],["git","add",*[s["path"] for s in sources]],
        ["git","-c","user.name=Fixture","-c","user.email=fixture@example.invalid","commit","-q","-m","synthetic source closure"]):
        subprocess.run(args,cwd=root,check=True,capture_output=True)
    revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=root).decode().strip()
    freeze=dict(schemaVersion=1,kind=full.KIND,sourceRevision=revision,binary=binary,parameters=parameters,sources=sources,
        qualityDecision=document("synthetic-decision.json",dict(controlledExternalAuthority=True)),
        manifest=manifest,inputs=inputs,model=model,vad=vad,qualified=False,L=None,outputDirectory="artifacts/central")
    document("freeze.json",freeze)
    return freeze


class PreflightCallbackTests(unittest.TestCase):
    def test_shared_actual_preflight_plus_full_print_and_freeze_fingerprint(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze=preflight_fixture(root)
            original=full.central.common.verify_binding
            def without_models(root,binding,*args,**kwargs):
                if binding["path"] in (freeze["model"]["path"],freeze["vad"]["path"]):return None
                return original(root,binding,*args,**kwargs)
            def external_authority(root,binding,fp,parameters):
                full.validate_parameters(parameters)
                return original(root,binding,fp,full.common.MAX_JSON,True)
            with mock.patch.object(full.central.common,"verify_binding",side_effect=without_models), \
                 mock.patch.object(full,"CENTRAL_MANIFEST_SHA",freeze["manifest"]["sha256"]), \
                 mock.patch.object(full,"validate_quality",side_effect=external_authority):
                plan=full.preflight(root/"freeze.json",root,"ref-cand")
                self.assertEqual(len(plan["cases"]),6)
                self.assertIn(root/"freeze.json",plan["fingerprints"])
                self.assertIn(root/"synthetic-decision.json",plan["fingerprints"])
                self.assertFalse(plan["outputRoot"].exists())
                (root/"freeze.json").write_bytes(b"changed after preflight")
                with self.assertRaises(ValueError):full.common.check_unchanged(plan)

    def test_main_delegates_explicitly_without_changing_legacy_globals(self):
        import contextlib
        import io
        with mock.patch.object(full,"preflight",return_value={}) as prepare, \
             mock.patch.object(full.central,"preflight",side_effect=AssertionError("legacy default called")):
            with contextlib.redirect_stdout(io.StringIO()):self.assertEqual(full.main(["freeze.json"]),0)
            prepare.assert_called_once()
        # Legacy callbacks are resolved at call time, preserving old patch seams.
        with mock.patch.object(full.central,"preflight",return_value={}) as legacy:
            with contextlib.redirect_stdout(io.StringIO()):self.assertEqual(full.central.main(["freeze.json"]),0)
            legacy.assert_called_once()


class ParameterIsolationTests(unittest.TestCase):
    def test_pair_accepts_only_explicit_full_prefix_mode_differences(self):
        import copy
        parameters={v:full_parameters(v=="candidate") for v in full.IDS}
        full.validate_parameters(parameters)
        for mutate in (
            lambda p:p["candidate"].update(configurationId="p06-six-case-pretranscription-candidate-v1"),
            lambda p:p["reference"]["fullPrefix"].update(candidate=True),
            lambda p:p["candidate"]["measurement"].update(totalRows=180),
            lambda p:p["candidate"]["measurement"].update(measuredRuns="1..30"),
            lambda p:p["candidate"]["runtime"]["fullParams"].update(audio_ctx=1024),
            lambda p:p["candidate"]["fullPrefix"].update(tailFrames=3199),
            lambda p:p["candidate"].update(sourceHashes={"foreign":"digest"}),
        ):
            wrong=copy.deepcopy(parameters)
            mutate(wrong)
            with self.assertRaises(ValueError):full.validate_parameters(wrong)


class FixedCorpusAuthorityTests(unittest.TestCase):
    def test_rebound_different_but_valid_six_case_manifest_cannot_leave_original_cases_unmeasured(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze=preflight_fixture(root)
            original_sha=freeze["manifest"]["sha256"]
            # The six audio formats, order and roles still satisfy the generic
            # central schema. Rebinding changed reference content must not turn
            # this into the central set excluded by the unchanged remainder94.
            path=root/freeze["manifest"]["path"]
            changed=json.loads(path.read_bytes())
            changed["cases"][0]["reference"]="another synthetically valid central reference"
            data=json.dumps(changed).encode()
            path.write_bytes(data)
            freeze["manifest"].update(bytes=len(data),sha256=full.common.sha256(data))
            (root/"freeze.json").write_text(json.dumps(freeze))
            original=full.central.common.verify_binding
            def without_models(root,binding,*args,**kwargs):
                if binding["path"] in (freeze["model"]["path"],freeze["vad"]["path"]):return None
                return original(root,binding,*args,**kwargs)
            with mock.patch.object(full.central.common,"verify_binding",side_effect=without_models), \
                 mock.patch.object(full,"validate_quality",return_value=None), \
                 mock.patch.object(full,"CENTRAL_MANIFEST_SHA",original_sha), \
                 self.assertRaises(ValueError):full.preflight(root/"freeze.json",root,"ref-cand")


class ProbePowerTests(unittest.TestCase):
    def test_actual_probe_battery_missing_or_error_cannot_be_overridden_by_wrapper_ac(self):
        import copy
        from test_pretranscription_central import results_sha
        plan,results,base=full_documents(True)
        for source in (dict(value="Battery Power",error=None),None,
                       dict(value="AC Power",error="controlled host observation failure"),
                       dict(value="AC Power")):
            metadata=copy.deepcopy(base)
            if source is None:metadata.pop("hostObservation")
            else:metadata["hostObservation"]["powerSource"]=source
            with self.subTest(power=source),self.assertRaises(ValueError):
                full.validate_documents(plan,"candidate",results,metadata,results_sha(results))
