import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from scripts.screening_quality.full_prefix_adapter import prepare

ROOT=Path(__file__).resolve().parents[2]


def write(root,name,value):
    path=root/name
    path.parent.mkdir(parents=True,exist_ok=True)
    data=(json.dumps(value,ensure_ascii=False,allow_nan=False)+"\n").encode()
    path.write_bytes(data)
    path.chmod(0o600)
    return dict(path=name,mode="0o600",bytes=len(data),sha256=hashlib.sha256(data).hexdigest())


class FullPrefixAdapterTests(unittest.TestCase):
    def test_a_bound_continuation_freeze_cannot_authorize_full_prefix_assembly(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            (root/"scripts").mkdir()
            shutil.copyfile(ROOT/"scripts/run-paced-remainder.py",root/"scripts/run-paced-remainder.py")
            binding=write(root,"freeze.json",dict(kind="prospective-pretranscription-continuation-v1"))
            with self.assertRaises(ValueError):prepare(root,binding,None,None)


# Synthetic files use the actual full-prefix protocol validator. Only the two
# prospective fixed-corpus hash constants are substituted; no model or evaluator
# is run, and no native scheduling success is inferred from these test doubles.
import copy
from unittest import mock
from test_full_prefix_screening import screen, valid_reports, parameter_fixture


def fixture(root):
    sources=[]
    for name in sorted(screen.OWN_SOURCES):
        target=root/name
        target.parent.mkdir(parents=True,exist_ok=True)
        data=(ROOT/name).read_bytes()
        target.write_bytes(data)
        target.chmod(0o644)
        sources.append(dict(path=name,bytes=len(data),mode="0o644",sha256=hashlib.sha256(data).hexdigest()))
    temporary=root/"template"
    temporary.mkdir()
    _,_,_,_,base=valid_reports(temporary)
    cases=[]
    for index in range(100):
        cases.append(dict(id="case-%03d"%index,language="de",split="acceptance",**{"class":"clean"},
            reference="" if index<6 else "synthetic reference",
            audio=dict(path="artifacts/input/%03d.wav"%index,frames=16000,sha256="a"*64)))
    manifest=dict(schemaVersion=1,unicodeVersion="13.0.0",cases=cases)
    selected=[dict({k:v for k,v in c.items() if k!="reference"},
        referenceUtf8Sha256=hashlib.sha256(c["reference"].encode()).hexdigest(),
        expectedOutcome="Empty" if not c["reference"] else "Local") for c in cases]
    manifest_binding=write(root,"manifest.json",manifest)
    selection_binding=write(root,"selection.json",dict(cases=selected))
    params={variant:parameter_fixture(variant=="FullPrefixCandidate") for variant in set(screen.VARIANTS)}
    for document in params.values():
        document["sourceHashes"]={"fixtureBoundary":dict(filename=sources[0]["path"],sha256=sources[0]["sha256"])}
    param_bindings={v:write(root,"params-%s.json"%v,d) for v,d in params.items()}
    output="artifacts/full-screen"
    blocks=[]
    for bi,variant in enumerate(screen.VARIANTS):
        entries=[]
        for ci,case in enumerate(cases):
            stem="%s/%d-%03d"%(output,bi,ci)
            entry=dict(id=case["id"],output=stem+".json",metadata=stem+".json.metadata.json",
                log=stem+".log",completion=stem+".completion.json")
            entry["command"]=["/usr/bin/time","-l",str(root/"target/release/examples/paced_runtime_probe"),
                str(root/screen.process.MODEL_PATH),"reference",str(root/case["audio"]["path"]),"de",
                str(root/param_bindings[variant]["path"]),str(root/entry["output"]),"1"]
            if not case["reference"]:entry["command"].append("--expect-empty")
            entries.append(entry)
        blocks.append(dict(name="block-%d"%bi,variant=variant,cases=entries))
    freeze=dict(schemaVersion=1,kind=screen.KIND,sourceRevision="f"*40,screeningId="test-synthetic-fresh",
        qualified=False,L=None,outputDirectory=output,manifest=manifest_binding,selection=selection_binding,
        parameters=param_bindings,sources=sources,binary=dict(path="target/release/examples/paced_runtime_probe"),
        models=[dict(path=screen.process.MODEL_PATH,sha256=screen.common.MODEL_SHA),
                dict(path=screen.process.VAD_PATH,sha256=screen.common.VAD_SHA)],blocks=blocks)
    frozen=write(root,"freeze.json",freeze)
    completed=[]
    for bi,block in enumerate(blocks):
        done=[]
        for ci,(case,planned) in enumerate(zip(cases,block["cases"])):
            slot=bi*100+ci+1
            candidate=block["variant"]=="FullPrefixCandidate"
            empty=ci<6
            meta=copy.deepcopy(base)
            meta.update(sourceRevision=freeze["sourceRevision"],processId=9000+slot,
                parameters=params[block["variant"]],parameterDocumentSha256=param_bindings[block["variant"]]["sha256"],
                model=dict(id="large-v3-turbo-q5_0",sha256=screen.common.MODEL_SHA),
                fullPrefixCandidate=candidate,expectedOutcome="Empty" if empty else "Local")
            results=[]
            logical=[]
            for row in meta["runs"]:
                evidence=row["jobTrace"]
                if not candidate or empty:
                    evidence["trace"]["jobs"]=evidence["trace"]["jobs"][-1:]
                    evidence["inputProofs"]=evidence["inputProofs"][-1:]
                    evidence.update(processedFramesSum=16000,checkedWorkerSpanSumNanos=99998)
                    receipt=evidence["trace"]["jobs"][0]["preparedReuse"]
                    receipt.update(outcome="decoded",originJobId=receipt["jobId"],decoderInvoked=True,nativeFullAttempted=True)
                    if empty:receipt.update(outcome="not-admitted",originJobId=None,preparedFrames=None,
                        preparedPcmSha256=None,decoderInvoked=False,nativeFullAttempted=False)
                    row.update(engineInputFrames=16000,engineElapsedNanos=50000,engineRtf=.00005)
                if empty:row.update(expectedOutcome="Empty",terminal="Empty",tTextOffsetNanos=None,stopToTextNanos=None)
                results.append(dict(id=row["id"],warmup=row["warmup"],status="ok",text="" if empty else "  synthetic / e\u0301\n"))
                logical.append(dict(id=case["id"],runId=row["id"],warmup=row["warmup"],status="ok",validated=True,
                    session=row["session"],jobCount=len(evidence["trace"]["jobs"]),
                    processedFramesSum=evidence["processedFramesSum"],recordingFrames=16000,
                    workerSpanNanos=evidence["checkedWorkerSpanSumNanos"],engineElapsedNanos=row["engineElapsedNanos"],
                    engineRtf=row["engineRtf"]))
            meta["jobTraceAfterCleanup"]=copy.deepcopy(meta["runs"][-1]["jobTrace"]["trace"])
            raw=[write(root,planned["output"],results),write(root,planned["metadata"],meta),write(root,planned["log"],{})]
            entry=dict(id=case["id"],globalSlot=slot,blockName=block["name"],variant=block["variant"],
                screeningId=freeze["screeningId"],freezeSha256=frozen["sha256"],status="ok",error=None,
                rows=logical,rawFiles=raw,actualProbePid=9000+slot,
                launch=dict(command=planned["command"],wrapperPid=5000+slot,exitCode=0,terminationConfirmed=True,
                    launchError=None,watchdogExpired=False),
                beforePower=dict(source="AC Power",error=None),afterPower=dict(source="AC Power",error=None))
            write(root,planned["completion"],entry)
            done.append(entry)
        completed.append(dict(name=block["name"],variant=block["variant"],cases=done))
    completion=dict(schemaVersion=1,kind="full-prefix-screening-execution-v1",sourceRevision=freeze["sourceRevision"],
        screeningId=freeze["screeningId"],freezeSha256=frozen["sha256"],completed=True,error=None,rowCount=800,
        powerAssertionEnded=True,qualified=False,L=None,blocks=completed)
    global_binding=write(root,output+"/completion.json",completion)
    return frozen,global_binding,freeze,completion


class FreshAuthorityTests(unittest.TestCase):
    def test_all_400_fresh_authorities_use_actual_overlap_validator_and_copy_800_literal_texts(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            frozen,global_binding,freeze,_=fixture(root)
            with mock.patch.object(screen.process,"MANIFEST_SHA",freeze["manifest"]["sha256"]), \
                 mock.patch.object(screen.process,"SELECTION_SHA",freeze["selection"]["sha256"]), \
                 mock.patch.object(screen.process,"validate_trace",side_effect=AssertionError("disjoint validator called")):
                manifest,arrays,provenance=prepare(root,frozen,global_binding,screen)
            self.assertEqual(len(manifest["cases"]),100)
            self.assertEqual(len(arrays),8)
            self.assertEqual(sum(map(len,arrays.values())),800)
            self.assertEqual(provenance["freshCases"],400)
            self.assertEqual(arrays[("block-0","warmup")][6]["text"],"  synthetic / e\u0301\n")
            self.assertFalse(any("adopted" in row or "epoch" in row["origin"] for row in provenance["slots"]))

    def test_tampered_raw_bytes_fail_before_pipeline_reread(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            frozen,global_binding,freeze,_=fixture(root)
            path=root/freeze["blocks"][0]["cases"][0]["output"]
            path.write_text('[{"text":"changed"}]')
            with mock.patch.object(screen.process,"MANIFEST_SHA",freeze["manifest"]["sha256"]), \
                 mock.patch.object(screen.process,"SELECTION_SHA",freeze["selection"]["sha256"]), \
                 self.assertRaises(ValueError):prepare(root,frozen,global_binding,screen)

    def test_source_bound_but_invalid_final_coverage_is_not_authorized_by_saved_rows(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            frozen,global_binding,freeze,completion=fixture(root)
            planned=freeze["blocks"][1]["cases"][6]
            entry=completion["blocks"][1]["cases"][6]
            metadata=json.loads((root/planned["metadata"]).read_bytes())
            metadata["runs"][1]["jobTrace"]["trace"]["jobs"][-1]["timelineEnd"]-=320
            entry["rawFiles"][1]=write(root,planned["metadata"],metadata)
            write(root,planned["completion"],entry)
            global_binding=write(root,global_binding["path"],completion)
            with mock.patch.object(screen.process,"MANIFEST_SHA",freeze["manifest"]["sha256"]), \
                 mock.patch.object(screen.process,"SELECTION_SHA",freeze["selection"]["sha256"]), \
                 self.assertRaises(ValueError):prepare(root,frozen,global_binding,screen)
