"""Synthetic resource authorities only: no real approval/model/native policy."""
import importlib.util
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("full_prefix_resource",ROOT/"scripts/run-full-prefix-resource.py")
runner=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class TransferTests(unittest.TestCase):
    def test_only_exact_known_addition_can_transfer_all47_sources(self):
        old={"logical/source-%02d.rs"%i:b"synthetic original\n" for i in range(46)}
        old[runner.SERVICE]=b"impl Service {\n"+runner.ANCHOR+b"\nfn legacy() {}\n}\n"
        new=dict(old)
        new[runner.SERVICE]=old[runner.SERVICE].replace(runner.ANCHOR,runner.INITIALIZER+runner.ANCHOR)
        proof=runner.validate_transfer(old,new)
        self.assertEqual(proof["historicalSourcesChecked"],47)
        self.assertEqual(proof["byteIdenticalSources"],46)
        for mutate in (lambda d:d.pop("logical/source-00.rs"),
                       lambda d:d.update({"logical/source-00.rs":b"changed body"}),
                       lambda d:d.update({runner.SERVICE:d[runner.SERVICE].replace(b"Self::spawn(initialize)",b"panic!()")}),
                       lambda d:d.update({runner.SERVICE:old[runner.SERVICE]})):
            changed=dict(new);mutate(changed)
            with self.assertRaises(ValueError):runner.validate_transfer(old,changed)


def workload():
    rows=[];receipts=[];ordinal=0;jid=0
    for i in range(105):
        warm=i<5;mi=i-5
        op="StopComplete" if warm or mi%3==0 else "CancelCapture" if mi%3==1 else "CancelDecoderEntry"
        sid=i+1;jobs=[]
        count=2 if op=="StopComplete" else 0 if op=="CancelCapture" else 1
        for n in range(count):
            jid+=1;ordinal+=1
            final=op=="StopComplete" and n==count-1;frames=16000 if final else 6400
            reuse=dict(session=sid,generation=1,jobId=jid,outcome="decoded" if op=="StopComplete" else "invalid-context",
                originJobId=jid if op=="StopComplete" else None,preparedFrames=16000 if op=="StopComplete" else None,
                preparedPcmSha256=[3]*32 if op=="StopComplete" else None,
                decoderInvoked=op=="StopComplete",nativeFullAttempted=op=="StopComplete")
            job=dict(session=sid,generation=1,jobId=jid,language="de",timelineStart=0,timelineEnd=frames,
                storageStart=0,storageEnd=frames,inputFrames=frames,audioReleasedAtSubmit=final,
                requiredStableFrames=frames if final else frames+4800,
                hostSubmitOffsetNanos=1,serviceAcceptedOffsetNanos=2,workerBeforeDecodeOffsetNanos=3,
                workerAfterDecodeOffsetNanos=4,workerSettledOffsetNanos=5,
                resultAdoptedOffsetNanos=7 if final else 6 if op=="StopComplete" else None,
                completionObservedOffsetNanos=6 if op=="StopComplete" else None,
                workerOutcome="decoded-ok" if op=="StopComplete" else "cancelled-after-decode",
                adoption="accepted" if op=="StopComplete" else None,preparedReuse=reuse if op=="StopComplete" else None)
            jobs.append(job)
            receipts.append(dict(ordinal=ordinal,identity=dict(job),pcmExact=True,decoderReturned=True,
                decodeContext=[sid,1,jid],decoderError=None if op=="StopComplete" else "Invalid or revoked decode context",
                preparedReuse=reuse))
        trace=dict(enabled=True,session=sid,maximumJobs=601,error=None,overflowEvents=0,firstOverflowJob=None,jobs=jobs,retiredJob=None)
        rows.append(dict(id="warmup-%03d"%(i+1) if warm else "measured-%03d"%(mi+1),warmup=warm,operation=op,
            session=sid,status="ok",error=None,terminal="Local" if op=="StopComplete" else "Cancelled",
            referenceMatch=True if op=="StopComplete" else None,textAbsent=op!="StopComplete",
            heldDecoderOrdinal=ordinal if op=="CancelDecoderEntry" else None,
            decoderStarted=count,decoderCompleted=count,decoderCompared=count,decoderMismatches=0,decoderError=None,
            controllerIdleAndWrapperIdle=True,ownerFreeConfirmedByNextStart=i<104,
            feedFrames=320 if op=="CancelCapture" else 16000,jobTrace=trace,
            fullStopCoverage=True if op=="StopComplete" else None,actualPublicationAdmissionValid=True,
            publicationCount=1 if op=="CancelCapture" else 50,feedTimingValid=True,maximumPublicationDeviationNanos=100))
    checkpoints=[]
    for n,i in enumerate([4]+list(range(5,105))):
        checkpoints.append(dict(checkpoint=dict(afterId=rows[i]["id"],status="ok",includeVmmap=n in (0,100),controllerIdleAndWrapperIdle=True),
            metrics=dict(rssBytes=1000+n,rssError=None,vmmapSummary="synthetic vmmap" if n in (0,100) else None,vmmapError=None,elapsedMicros=1),rssDeltaFromWarmupBytes=n))
    params={"synthetic":"not an actual approval"}
    plan=dict(mode="FullPrefixCandidate",parameters=params,parameterSha256="a"*64,sourceRevision="f"*40,
        model=dict(id="large-v3-turbo-q5_0",sha256="b"*64,bytes=1),fixture=dict(sha256="c"*64,frames=16000,sampleRate=16000,channels=1),
        referenceSha256="d"*64,preparedSha256="e"*64)
    report=dict(schemaVersion=1,kind="runtime-lifecycle-resource-workload",experimentalMode=plan["mode"],parameters=params,
        parameterDocumentSha256=plan["parameterSha256"],sourceRevision=plan["sourceRevision"],model=plan["model"],fixture=plan["fixture"],
        referenceSha256=plan["referenceSha256"],preparedFixtureFrames=16000,preparedFixturePcmF32LeSha256=plan["preparedSha256"],
        host=dict(processId=123,debugAssertions=False,os="macos",architecture="aarch64"),behaviorPassed=True,resourceQualified=False,
        setupTimingPassed=True,loadError=None,metricsErrorCount=0,decoderDiagnosticsError=None,
        workload=dict(behaviorPassed=True,setupTimingPassed=True,firstError=None,rows=rows,decoderCallsStarted=ordinal,decoderCallsCompleted=ordinal),
        checkpoints=checkpoints,cleanup=dict(joined=True,error=None,elapsedMicros=1),
        decoderAfterCleanup=dict(started=ordinal,completed=ordinal,compared=ordinal,mismatches=0,active=False,armed=False,lastError=None),
        observationAfterCleanup=dict(mode=plan["mode"],valid=True,validationError=None,afterJoinTrace=rows[-1]["jobTrace"],afterJoinTraceError=None,
            actualDecoderReceipts=receipts,receiptReadError=None))
    return plan,report


class WorkloadTests(unittest.TestCase):
    def test_actual105_shape_preserves_scoped_calls_cancel_and_metrics(self):
        plan,report=workload()
        summary=runner.validate_workload(plan,report)
        self.assertEqual(summary["rows"],105)
        self.assertEqual(summary["stopRows"],39)
        self.assertEqual(summary["cancelRows"],66)
        self.assertEqual(summary["followStarts"],104)
        self.assertEqual(summary["rssPoints"],101)
        self.assertEqual(summary["vmmapPoints"],2)
        self.assertEqual(summary["actualCalls"],111)

    def test_missing_metrics_wrong_scopes_geometry_and_join_do_not_pass(self):
        import copy
        plan,base=workload()
        mutations=(lambda r:r["checkpoints"].pop(),
            lambda r:r["checkpoints"][0]["metrics"].update(rssBytes=None,rssError="synthetic failure"),
            lambda r:r["checkpoints"][-1]["metrics"].update(vmmapSummary=""),
            lambda r:r["cleanup"].update(joined=False),
            lambda r:r["workload"]["rows"][7]["jobTrace"]["jobs"][0].update(timelineStart=320),
            lambda r:r["observationAfterCleanup"]["actualDecoderReceipts"][-1]["preparedReuse"].update(jobId=0),
            lambda r:r["workload"]["rows"][0].update(ownerFreeConfirmedByNextStart=False),
            lambda r:r["observationAfterCleanup"]["actualDecoderReceipts"][0].update(decodeContext=[99,1,1]),
            lambda r:r["workload"]["rows"][0].update(decoderStarted=1),
            lambda r:r["workload"]["rows"][1].update(status="unavailable"),
            lambda r:r["workload"]["rows"][0]["jobTrace"]["jobs"][0].update(preparedReuse=None),
            lambda r:r["observationAfterCleanup"]["actualDecoderReceipts"][0]["identity"].update(audioReleasedAtSubmit=True),
            lambda r:r["workload"]["rows"][8]["jobTrace"].update(retiredJob=dict(r["workload"]["rows"][7]["jobTrace"]["jobs"][0],jobId=999999)))
        for index,mutate in enumerate(mutations):
            with self.subTest(index=index):
                value=copy.deepcopy(base);mutate(value)
                with self.assertRaises((ValueError,KeyError)):runner.validate_workload(plan,value)


class CommandTests(unittest.TestCase):
    def test_only_explicit_resource_mode_is_passed_to_known_probe(self):
        plan=dict(binary="/bound/runtime_resource_probe",modelPath="/bound/model",wavPath="/bound/input",referencePath="/bound/reference",
            parameterPath="/bound/params",paths=dict(output="/bound/new"),mode="FullPrefixCandidate")
        self.assertEqual(runner.command_for(plan),["/usr/bin/time","-l","/bound/runtime_resource_probe","/bound/model","reference",
            "/bound/input","/bound/reference","/bound/params","/bound/new","--full-prefix-candidate"])


class AuthorityTests(unittest.TestCase):
    def test_unmodified_strict_decision_rejects_initializer_in_current_tree(self):
        import hashlib
        from test_full_prefix_quality_decision import ApprovalTests,decision
        fixture=ApprovalTests();fixture.setUp();self.addCleanup(fixture.doCleanups)
        old=b"impl S {\n"+runner.ANCHOR+b"\nfn original(){}\n}\n"
        service=fixture.write(runner.SERVICE,old.decode())
        old_name=fixture.sources[-1]["path"];fixture.sources[-1]=service
        for parameters in fixture.parameters.values():
            pin=next(p for p in parameters["sources"] if p["filename"]==old_name)
            pin.update(filename=runner.SERVICE,sha256=service["sha256"])
        for role,p in fixture.parameters.items():
            fixture.freeze["parameters"]["FullPrefix"+("Candidate" if role=="candidate" else "Reference")]=fixture.write(role+"-parameters.json",p)
        bound=fixture.binding()
        self.assertEqual(decision.validate(fixture.root,bound,{},fixture.parameters)["compiledQualitySourcesChecked"],47)
        (fixture.root/runner.SERVICE).write_bytes(old.replace(runner.ANCHOR,runner.INITIALIZER+runner.ANCHOR))
        with self.assertRaises(ValueError):decision.validate(fixture.root,bound,{},fixture.parameters)


class PrinterTests(unittest.TestCase):
    def test_actual_bound_printer_flag_and_document_are_checked_without_model(self):
        import tempfile
        import json
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"runtime_resource_probe"
            path.write_text('#!/usr/bin/python3\nimport sys,json\nassert sys.argv[1:]==["--print-full-prefix-candidate-parameters"]\nprint(json.dumps({"synthetic":True}))\n')
            path.chmod(0o700)
            runner.verify_printed_template(path,"FullPrefixCandidate",{"synthetic":True})
            with self.assertRaises(ValueError):runner.verify_printed_template(path,"FullPrefixCandidate",{"synthetic":False})


class ProcessTests(unittest.TestCase):
    def test_oversized_owned_log_fails_and_retains_terminal_evidence(self):
        import sys,tempfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            command=[sys.executable,"-c","import sys;sys.stdout.write('x'*(17*1024*1024));sys.stdout.flush()"]
            report=runner.run_owned(dict(root=root,sourceRevision="f"*40),command,root/"oversized.log",watchdog=3)
            self.assertIsNotNone(report["launchError"])
            self.assertTrue(report["terminationConfirmed"])
            self.assertTrue(runner.process.group_ended(report["wrapperPid"]))

    def test_denied_signal_preserves_assertion_and_still_cleans_foreign_fixture(self):
        from unittest.mock import Mock, patch
        foreign = Mock()
        foreign.poll.return_value = None
        report = {"watchdogExpired": True, "terminationConfirmed": False, "wrapperPid": 12345}
        case = ProcessTests("test_real_owned_group_timeout_is_reaped_without_touching_foreign_fixture")
        result = unittest.TestResult()
        with patch("subprocess.Popen", return_value=foreign), \
             patch.object(runner, "run_owned", return_value=report), \
             patch("os.killpg", side_effect=PermissionError("controlled denied signal")), \
             patch.object(runner.process, "stop_owned") as cleanup:
            case.run(result)
        cleanup.assert_called_once_with(foreign)
        self.assertEqual(len(result.failures), 1)
        self.assertIn("AssertionError", result.failures[0][1])
        self.assertEqual(len(result.errors), 1)
        self.assertIn("controlled denied signal", result.errors[0][1])

    def test_unconfirmed_foreign_cleanup_is_a_separate_failure(self):
        from unittest.mock import Mock, patch
        foreign = Mock()
        foreign.poll.return_value = None
        report = {"watchdogExpired": True, "terminationConfirmed": True, "wrapperPid": 12345}
        case = ProcessTests("test_real_owned_group_timeout_is_reaped_without_touching_foreign_fixture")
        result = unittest.TestResult()
        with patch("subprocess.Popen", return_value=foreign), \
             patch.object(runner, "run_owned", return_value=report), \
             patch.object(runner.process, "group_ended", return_value=True), \
             patch.object(runner.process, "stop_owned", return_value=False):
            case.run(result)
        self.assertEqual(len(result.failures), 1)
        self.assertIn("foreign fixture termination unconfirmed", result.failures[0][1])

    def test_real_owned_group_timeout_is_reaped_without_touching_foreign_fixture(self):
        import os
        import subprocess
        import sys
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            foreign=subprocess.Popen([sys.executable,"-c","import time;time.sleep(20)"],start_new_session=True)
            # Register independently: unittest runs every cleanup even if another
            # cleanup raises, and retains the original assertion separately.
            def cleanup_foreign():
                self.assertTrue(runner.process.stop_owned(foreign), "foreign fixture termination unconfirmed")
            self.addCleanup(cleanup_foreign)
            command=[sys.executable,"-c","import signal,subprocess,sys,time;signal.signal(signal.SIGCHLD,signal.SIG_IGN);subprocess.Popen([sys.executable,'-c','import time;time.sleep(20)']);time.sleep(20)"]
            report=runner.run_owned(dict(root=root,sourceRevision="f"*40),command,root/"owned.log",watchdog=.15)
            def cleanup_owned():
                if not report["terminationConfirmed"]:
                    try:os.killpg(report["wrapperPid"],9)
                    except ProcessLookupError:pass
            self.addCleanup(cleanup_owned)
            self.assertTrue(report["watchdogExpired"])
            self.assertTrue(report["terminationConfirmed"])
            self.assertTrue(runner.process.group_ended(report["wrapperPid"]))
            self.assertIsNone(foreign.poll())



class ExecutionTests(unittest.TestCase):
    def test_complete_and_final_power_failure_preserve105_slots_without_subset_summary(self):
        import tempfile
        import copy
        for final_ac in (True,False):
            with self.subTest(final_ac=final_ac),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);plan,raw=workload()
                output=root/"new";plan.update(root=root,output=output,paths=dict(output=output/"workload.json",log=output/"command.log"),
                    binary=root/"runtime_resource_probe",modelPath=root/"model",wavPath=root/"wav",referencePath=root/"reference",parameterPath=root/"params",
                    freezeBytes=b"synthetic freeze",freezeSha256="0"*64,fingerprints={},extraFingerprints=[])
                calls=[]
                def power():
                    calls.append(1)
                    return dict(source="AC Power" if len(calls)==1 or final_ac else "Battery Power",error=None)
                def launch(plan):
                    runner.common.write_private(plan["paths"]["output"],raw)
                    runner.common.write_private_bytes(plan["paths"]["log"],b"synthetic command status only\n")
                    return dict(exitCode=0,terminationConfirmed=True,watchdogExpired=False,launchError=None,wrapperPid=456)
                result=runner.execute(plan,launch=launch,power=power,finish=lambda:True,host=lambda:dict(cpu=dict(value="synthetic",error=None)))
                self.assertIs(result["completed"],final_ac)
                self.assertEqual(len(result["rows"]),105)
                self.assertEqual(len(result["rawFiles"]),4)
                self.assertTrue(all(b.get("sha256") for b in result["rawFiles"]))
                self.assertIs(result["summary"] is not None,final_ac)
                self.assertTrue(all(r["validated"] is final_ac for r in result["rows"]))
                with self.assertRaises(FileExistsError):runner.execute(plan,launch=launch,power=power,finish=lambda:True)

class BaselineTests(unittest.TestCase):
    def test_only_four_bound_mode_specific_original_texts_supply_baseline(self):
        import tempfile,json,os,hashlib,copy
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);text='Synthetic baseline, not a qualification.'
            fixture=dict(sha256='c'*64,frames=80000)
            source=dict(id='de-five',language='de',audio=fixture)
            freeze=dict(blocks=[])
            completion=dict(blocks=[])
            for i,variant in enumerate(('FullPrefixReference','FullPrefixCandidate','FullPrefixCandidate','FullPrefixReference')):
                path='artifacts/B%d.json'%i
                data=json.dumps([dict(id=run,warmup=warm,status='ok',text=text) for run,warm in (('warmup',True),('measured-001',False))]).encode()
                target=root/path;target.parent.mkdir(exist_ok=True);target.write_bytes(data);os.chmod(target,0o600)
                binding=dict(path='/original/repo/'+path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),mode='0o600')
                frozen=dict(id='de-five',output=path,command=['unused']*8+['/original/repo/'+path])
                freeze['blocks'].append(dict(name='B%d'%i,variant=variant,cases=[frozen]))
                completion['blocks'].append(dict(name='B%d'%i,variant=variant,cases=[dict(id='de-five',rawFiles=[binding])]))
            plan=dict(mode='FullPrefixCandidate',fixtureCaseId='de-five',fixture=fixture,reference=dict(sha256=hashlib.sha256(text.encode()).hexdigest()))
            proof=runner.verify_baseline(root,plan,freeze,completion,dict(cases=[source]),{})
            self.assertEqual(proof['originalOutputsChecked'],4)
            self.assertFalse(proof['accuracyInferred'])
            for mutate in (lambda f:f.update(mode='wrong'),lambda f:f['fixture'].update(frames=1),lambda f:f['reference'].update(sha256='0'*64)):
                changed=copy.deepcopy(plan);mutate(changed)
                with self.assertRaises(ValueError):runner.verify_baseline(root,changed,freeze,completion,dict(cases=[source]),{})
            source['language']='en'
            with self.assertRaises(ValueError):runner.verify_baseline(root,plan,freeze,completion,dict(cases=[source]),{})

class DualRootTests(unittest.TestCase):
    def test_real_historical_and_derivative_git_trees_keep_strict_authority_separate(self):
        import copy,hashlib,json,os,shutil,subprocess,tempfile
        from test_full_prefix_quality_decision import ApprovalTests
        fixture=ApprovalTests();fixture.setUp();self.addCleanup(fixture.doCleanups)
        required=[runner.SERVICE,'src-tauri/src/audio.rs','src-tauri/src/controller.rs','src-tauri/src/runtime.rs',
            'src-tauri/src/transcription.rs','src-tauri/src/transcription/prefix_cache.rs',
            'src-tauri/src/transcription/prefix_cache/evidence.rs','src-tauri/src/audio/full_prefix.rs','src-tauri/src/controller/full_prefix.rs']
        names=required+['sources/synthetic-%02d.rs'%i for i in range(38)]
        fixture.sources=[]
        for name in names:
            fixture.sources.append(fixture.write(name,'impl S {\n'+runner.ANCHOR.decode()+'\nfn original(){}\n}\n' if name==runner.SERVICE else 'synthetic original '+name))
        fixture.freeze['sources']=fixture.sources
        pins=[dict(filename=b['path'],sha256=b['sha256']) for b in fixture.sources]
        for role,p in fixture.parameters.items():
            p['sources']=copy.deepcopy(pins)
            fixture.freeze['parameters']['FullPrefix'+('Candidate' if role=='candidate' else 'Reference')]=fixture.write(role+'-parameters.json',p)
        baseline='Synthetic mode output only.';fixture_audio=dict(sha256='c'*64,frames=80000)
        fixture.freeze['manifest']=fixture.write('manifest.json',dict(cases=[dict(id='case-000',language='de',audio=fixture_audio)]))
        for frozen,observed in zip(fixture.freeze['blocks'],fixture.completion['blocks']):
            path='artifacts/'+frozen['name']+'-result.json'
            raw=fixture.write(path,[dict(id=r,warmup=w,status='ok',text=baseline) for r,w in (('warmup',True),('measured-001',False))])
            frozen['cases'][0].update(output=path,command=['unused']*8+['/recorded/repo/'+path])
            observed['cases'][0]['rawFiles']=[dict(raw,path='/recorded/repo/'+path)]
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();qroot=root/'artifacts/authority';rroot=root/'artifacts/resource'
            def git(*args):
                return subprocess.check_output(['git']+list(args),cwd=root,stderr=subprocess.DEVNULL).decode().strip()
            git('init','-q');git('config','user.email','synthetic@example.invalid');git('config','user.name','Synthetic Test')
            for b in fixture.sources:
                target=root/b['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((fixture.root/b['path']).read_bytes())
            git('add','.');git('commit','-qm','synthetic historical source');old=git('rev-parse','HEAD')
            fixture.freeze['sourceRevision']=old;fixture.completion['sourceRevision']=old;fixture.report['sourceRevision']=old
            decision_binding=fixture.binding();shutil.copytree(fixture.root,qroot)
            service=root/runner.SERVICE
            service.write_bytes(service.read_bytes().replace(runner.ANCHOR,runner.INITIALIZER+runner.ANCHOR))
            git('add',runner.SERVICE);git('commit','-qm','synthetic reviewed additive API');new=git('rev-parse','HEAD')
            derivatives=[]
            for b in fixture.sources:
                target=rroot/b['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((root/b['path']).read_bytes());target.chmod(0o600)
                derivatives.append(dict(path=b['path'],bytes=target.stat().st_size,mode='0o600',sha256=hashlib.sha256(target.read_bytes()).hexdigest()))
            for name in ('scripts/screening_quality/decision.py','scripts/run-paced-remainder.py'):
                target=root/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/name,target)
            def bind(name,value):
                data=json.dumps(value).encode();target=root/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data);target.chmod(0o600)
                return dict(path=name,bytes=len(data),mode='0o600',sha256=hashlib.sha256(data).hexdigest())
            transfer=bind('artifacts/transfer.json',dict(schemaVersion=1,kind='full-prefix-resource-source-transfer-v1',historicalRevision=old,resourceRevision=new,
                qualityDecisionSha256=decision_binding['sha256'],qualityAuthorityRoot='artifacts/authority',resourceSourceRoot='artifacts/resource',sources=derivatives))
            freeze=dict(qualityAuthorityRoot='artifacts/authority',resourceSourceRoot='artifacts/resource',qualityDecision=decision_binding,sourceTransfer=transfer,
                sourceRevision=new,resourceSources=derivatives,mode='FullPrefixCandidate',binary=dict(sha256='b'*64),parameters=dict(sha256='a'*64),
                fixture=fixture_audio,fixtureCaseId='case-000',reference=dict(sha256=hashlib.sha256(baseline.encode()).hexdigest()),preparedPcmSha256='e'*64)
            approval=dict(schemaVersion=1,kind='full-prefix-resource-measurement-approval-v1',measurementAllowed=True,productionActivated=False,resourceQualified=False,
                qualityDecisionSha256=decision_binding['sha256'],sourceTransferSha256=transfer['sha256'],sourceRevision=new,binarySha256='b'*64,parameterSha256='a'*64,
                mode=freeze['mode'],fixtureSha256=fixture_audio['sha256'],referenceSha256=freeze['reference']['sha256'],preparedPcmSha256='e'*64)
            approval['staticEligibilityCatalogue']=bind('artifacts/static.json',dict(schemaVersion=1,kind='static-confirmed-pause-input-catalogue-v1',records=[dict(
                id='case-000',sha256='c'*64,frames=80000,sampleRate=16000,channels=1,potentialDistinctOffers=[dict(cutFrame=68480,earliest20msPublicationEndFrame=73280)])]))
            freeze['resourceMeasurementApproval']=bind('artifacts/measurement.json',approval)
            parameters=dict(runtimeBaseline=dict(runtime=fixture.runtime),sources=[dict(filename=b['path'],sha256=b['sha256']) for b in derivatives])
            proof,roots=runner._approval_and_transfer(root,freeze,parameters,{})
            self.assertEqual(proof['transfer']['byteIdenticalSources'],46)
            self.assertEqual(proof['baseline']['originalOutputsChecked'],4)
            self.assertEqual(len(roots),2)
            # A real modified derivative cannot masquerade as either original or approved addition.
            (rroot/runner.SERVICE).write_bytes(b'changed body')
            with self.assertRaises(ValueError):runner._approval_and_transfer(root,freeze,parameters,{})

class EligibilityTests(unittest.TestCase):
    def test_catalogue_is_only_matching_static_eligibility_not_runtime_evidence(self):
        import copy
        record=dict(id='de-five',sha256='c'*64,frames=80000,sampleRate=16000,channels=1,
            potentialDistinctOffers=[dict(cutFrame=68480,earliest20msPublicationEndFrame=73280)])
        catalogue=dict(schemaVersion=1,kind='static-confirmed-pause-input-catalogue-v1',records=[record])
        freeze=dict(fixtureCaseId='de-five',fixture=dict(sha256='c'*64,frames=80000))
        proof=runner.validate_eligibility(catalogue,freeze)
        self.assertEqual(proof['eligibleCutCount'],1)
        self.assertFalse(proof['actualRuntimeOfferProved'])
        for mutate in (lambda r:r.update(sha256='0'*64),lambda r:r.update(frames=1),
            lambda r:r.update(potentialDistinctOffers=[]),lambda r:r['potentialDistinctOffers'][0].update(earliest20msPublicationEndFrame=80000)):
            changed=copy.deepcopy(catalogue);mutate(changed['records'][0])
            with self.assertRaises(ValueError):runner.validate_eligibility(changed,freeze)

class CancelSettlementTests(unittest.TestCase):
    def test_actual_held_cancel_rejects_unknown_or_missing_worker_outcome(self):
        for outcome in ('unverified-outcome',None,'decoded-ok','cancelled-before-decode'):
            with self.subTest(outcome=outcome):
                plan,report=workload()
                report['workload']['rows'][7]['jobTrace']['jobs'][0]['workerOutcome']=outcome
                with self.assertRaises(ValueError):runner.validate_workload(plan,report)

    def test_pending_cancel_settlement_cannot_precede_service_acceptance(self):
        import copy
        plan,report=workload()
        jobs=report['workload']['rows'][103]['jobTrace']['jobs']
        pending_id=jobs[0]['jobId']+1
        visited=set()
        def shift(value):
            if isinstance(value,(dict,list)):
                if id(value) in visited:return
                visited.add(id(value))
            if isinstance(value,dict):
                for key,item in value.items():
                    if key in ('jobId','originJobId') and type(item) is int and item>=pending_id:value[key]+=1
                    elif key=='decodeContext' and item[2]>=pending_id:item[2]+=1
                    else:shift(item)
            elif isinstance(value,list):
                for item in value:shift(item)
        shift(report)
        pending=copy.deepcopy(jobs[0])
        pending.update(jobId=pending_id,workerBeforeDecodeOffsetNanos=None,workerAfterDecodeOffsetNanos=None,
            workerSettledOffsetNanos=3,workerOutcome='cancelled-pending-no-decode',preparedReuse=None)
        jobs.append(pending)
        self.assertEqual(runner.validate_workload(plan,report)['actualCalls'],111)
        pending['workerSettledOffsetNanos']=1
        with self.assertRaises(ValueError):runner.validate_workload(plan,report)
