import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("continuation",ROOT/"scripts/run-pretranscription-continuation.py")
tool=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(tool)


def originals():
    return (json.loads((ROOT/"tests/fixtures/corpus/p06-pretranscription-screening-run-v2.json").read_bytes()),
            json.loads((ROOT/"docs/verification/p06-pretranscription-screening-v2-failed.json").read_bytes()))


class LedgerTests(unittest.TestCase):
    def test_only_exact_whole_prefix185_is_eligible_never_failed186_or_v3(self):
        old,archive=originals()
        for change in (
            lambda a:a["cases"][0].update(status="failed"),
            lambda a:a["cases"][0]["rows"].pop(),
            lambda a:a["cases"][185].update(status="ok"),
            lambda a:a["cases"].pop(),
        ):
            altered=copy.deepcopy(archive);change(altered)
            with self.assertRaises(ValueError):
                tool.build_ledger(old,altered)


class ExecutionTests(unittest.TestCase):
    def test_failed186_repeats_both_phases_and_stops_all_later_starts(self):
        from unittest import mock
        import tempfile
        old,archive=originals()
        adoption=tool.build_ledger(old,archive)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            blocks=copy.deepcopy(old["blocks"])
            for b in blocks:
                for c in b["cases"]:
                    for k in ("output","metadata","log","completion"):
                        c[k]="new/"+Path(c[k]).name
            entries=copy.deepcopy(archive["cases"][:185])
            plan=dict(root=root,output=root/"new",blocks=blocks,adoption=entries,continuationId="controlled-new",
                      oldSourceRevision=tool.OLD_REVISION,sourceRevision="a"*40,oldFreezeSha256=tool.OLD_FREEZE_SHA,adoptionLedger=adoption)
            calls=[]
            def launch(case,paths):
                calls.append(case["id"])
                return dict(exitCode=1,terminationConfirmed=True,launchError="controlled new failure")
            ac=dict(source="AC Power",error=None)
            with mock.patch.object(tool.screen,"power_snapshot",return_value=ac):
                result=tool.execute(plan,launch)
            self.assertEqual(calls,["constructed-de-masked-300s"])
            allcases=[c for b in result["blocks"] for c in b["cases"]]
            self.assertEqual(len(allcases),400)
            self.assertEqual(sum(len(c["rows"]) for c in allcases),800)
            self.assertTrue(all(c["adopted"] for c in allcases[:185]))
            self.assertFalse(allcases[185]["adopted"])
            self.assertEqual(allcases[185]["status"],"failed")
            self.assertTrue(all(c["status"]=="unavailable" for c in allcases[186:]))
            self.assertFalse(result["completed"])
            self.assertFalse(result["qualified"])

class BoundaryTests(unittest.TestCase):
    def test_reordered_duplicate_or_shared_asset_change_is_rejected(self):
        old,archive=originals()
        for index in (0,100,184):
            bad=copy.deepcopy(archive)
            bad["cases"][index],bad["cases"][index+1]=bad["cases"][index+1],bad["cases"][index]
            with self.assertRaises(ValueError):tool.build_ledger(old,bad)
        for key in ("binary","parameters","models","inputs"):
            bad=copy.deepcopy(old);bad[key]={"changed":True}
            with self.assertRaises(ValueError):tool.common_assets(old,bad)

    def test_original_hash_tamper_missing_and_private_no_clobber(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            bound=tool.prepared_bind(root,"old.json",{"status":"ok"})
            self.assertEqual(bound["mode"],"0o600")
            before=(root/"old.json").read_bytes()
            with self.assertRaises((OSError,ValueError)):
                tool.prepared_bind(root,"old.json",{"changed":True})
            self.assertEqual((root/"old.json").read_bytes(),before)
            refs=[]
            for n in range(3):refs.append(tool.prepared_bind(root,"raw%d.json"%n,{"n":n}))
            reference=dict(completion=bound,rawFiles=refs)
            self.assertEqual(tool.verify_ledger_files(root,reference,{}),{"status":"ok"})
            (root/"raw1.json").write_bytes(b"tampered")
            with self.assertRaises(ValueError):tool.verify_ledger_files(root,reference,{})
            (root/"raw1.json").unlink()
            with self.assertRaises((ValueError,OSError)):tool.verify_ledger_files(root,reference,{})

    def test_excluded_epoch_requires_confirmed_cleanup_but_allows_planned_interruption(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            frozen=tool.prepared_bind(root,"freeze.json",{"runId":"p06-pretranscription-screening-v3"})
            launch=tool.prepared_bind(root,"launch.json",{"freezeSha256":frozen["sha256"]})
            body=dict(freezeSha256=frozen["sha256"],powerAssertionEnded=True,completed=False,
                      blocks=[dict(cases=[{} for _ in range(100)]) for _ in range(4)])
            body["blocks"][0]["cases"][0]={"launch":{"terminationConfirmed":True,"wrapperPid":99}}
            completion=tool.prepared_bind(root,"done.json",body)
            excluded=dict(freeze=frozen,launch=launch,completion=completion)
            tool.verify_excluded_epoch(root,excluded,{})
            body["blocks"][0]["cases"][0]["launch"]["terminationConfirmed"]=False
            excluded["completion"]=tool.prepared_bind(root,"bad.json",body)
            with self.assertRaises(ValueError):tool.verify_excluded_epoch(root,excluded,{})

    def test_success_starts_exact215_preserves_all800_and_old_source(self):
        import tempfile
        from unittest import mock
        old,archive=originals();ledger=tool.build_ledger(old,archive)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve();blocks=copy.deepcopy(old["blocks"])
            for b in blocks:
                for c in b["cases"]:
                    for k in ("output","metadata","log","completion"):c[k]="new/"+Path(c[k]).name
            plan=dict(root=root,output=root/"new",blocks=blocks,adoption=copy.deepcopy(archive["cases"][:185]),
                      continuationId="new-epoch",oldSourceRevision=tool.OLD_REVISION,sourceRevision="a"*40,
                      oldFreezeSha256=tool.OLD_FREEZE_SHA,adoptionLedger=ledger)
            calls=[]
            def launch(case,paths):
                calls.append(case["id"])
                for k in ("output","metadata","log"):tool.common.write_private(paths[k],{"synthetic":True})
                return dict(exitCode=0,terminationConfirmed=True,launchError=None,startedUtc=str(len(calls)))
            def validate(plan,block,case,paths):
                return dict(errors=[],rows=tool.screen.canonical_rows(case,"ok"),processId=123)
            with mock.patch.object(tool.screen,"power_snapshot",return_value=dict(source="AC Power",error=None)),mock.patch.object(tool.screen,"validate_case",side_effect=validate):
                result=tool.execute(plan,launch,lambda:True)
            entries=[c for b in result["blocks"] for c in b["cases"]]
            self.assertEqual(calls,[c["id"] for b,i,c in tool.slots(old)][185:])
            self.assertEqual(len(calls),215);self.assertEqual(result["rowCount"],800)
            self.assertTrue(result["completed"]);self.assertTrue(result["powerAssertionEnded"])
            self.assertEqual(entries[0]["origin"]["sourceRevision"],tool.OLD_REVISION)
            self.assertEqual(entries[185]["origin"]["sourceRevision"],"a"*40)
            self.assertEqual(sum(c["adopted"] for c in entries),185)

class HistoricalValidationTests(unittest.TestCase):
    def test_actual_saved_report_boundary_rejects_wrong_old_source_or_changed_rows(self):
        import tempfile
        fixture_spec=importlib.util.spec_from_file_location("screen_fixture",Path(__file__).with_name("test_pretranscription_screening.py"))
        fixture=importlib.util.module_from_spec(fixture_spec);fixture_spec.loader.exec_module(fixture)
        with tempfile.TemporaryDirectory() as directory:
            plan,block,case,paths,meta=fixture.valid_reports(Path(directory))
            plan["sourceRevision"]=meta["sourceRevision"]=tool.OLD_REVISION
            paths["metadata"].write_text(json.dumps(meta))
            accepted=tool.screen.validate_case(plan,block,case,paths)
            self.assertEqual(accepted["errors"],[])
            entry=dict(status="ok",error=None,rows=accepted["rows"],actualProbePid=42,
                       beforePower=dict(source="AC Power",error=None),afterPower=dict(source="AC Power",error=None),
                       launch=dict(exitCode=0,terminationConfirmed=True,launchError=None))
            tool.validate_original_case(plan,block,case,entry,paths)
            meta["sourceRevision"]="a"*40
            paths["metadata"].write_text(json.dumps(meta))
            with self.assertRaises(ValueError):tool.validate_original_case(plan,block,case,entry,paths)
            meta["sourceRevision"]=tool.OLD_REVISION
            paths["metadata"].write_text(json.dumps(meta))
            changed=copy.deepcopy(entry);changed["rows"][1]["jobCount"]=1
            with self.assertRaises(ValueError):tool.validate_original_case(plan,block,case,changed,paths)

class ClearanceTests(unittest.TestCase):
    def test_supplement_matches_exact_original_inventory_and_live_absence(self):
        import tempfile
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory).resolve()
            freeze=tool.prepared_bind(root,"f.json",{"runId":"p06-pretranscription-screening-v3"})
            launch=tool.prepared_bind(root,"l.json",dict(freezeSha256=freeze["sha256"],runnerPid=10,caffeinatePid=11))
            done=dict(freezeSha256=freeze["sha256"],powerAssertionEnded=True,completed=False,
                      blocks=[dict(cases=[{} for _ in range(100)]) for _ in range(4)])
            done["blocks"][0]["cases"][0]=dict(actualProbePid=13,launch=dict(wrapperPid=12,terminationConfirmed=False))
            completion=tool.prepared_bind(root,"c.json",done)
            excluded=dict(freeze=freeze,launch=launch,completion=completion)
            clearance=dict(schemaVersion=1,kind="p06-v3-post-stop-owner-clearance",authority="Root independent read-only OS observation",
                           v3CompletionSha256=completion["sha256"],v3LaunchSha256=launch["sha256"],
                           observedUtcISO="2026-10-18T13:30:00Z",knownPids=[10,11,12,13],processGroupIds=[12],allKnownPidsAbsent=True,allOwnedGroupsAbsent=True)
            excluded["ownerClearance"]=tool.prepared_bind(root,"clear.json",clearance)
            with mock.patch.object(tool,"definitely_absent",return_value=True,create=True) as absent:
                result=tool.verify_excluded_epoch(root,excluded,{})
                self.assertFalse(result["completion"]["blocks"][0]["cases"][0]["launch"]["terminationConfirmed"])
                self.assertEqual(absent.call_count,5)
            with mock.patch.object(tool,"definitely_absent",return_value=False,create=True):
                with self.assertRaises(ValueError):tool.verify_excluded_epoch(root,excluded,{})
            for key,value in (("knownPids",[10,11,12,13,99]),("processGroupIds",[]),("v3CompletionSha256","0"*64)):
                bad=dict(clearance,**{key:value})
                excluded["ownerClearance"]=tool.prepared_bind(root,key+".json",bad)
                with self.assertRaises(ValueError):tool.verify_excluded_epoch(root,excluded,{})

    def test_zero_signal_only_esrch_proves_absence_for_pid_and_group(self):
        from unittest import mock
        for group,name in ((False,"kill"),(True,"killpg")):
            for error,expected in ((ProcessLookupError(),True),(PermissionError(),False),(OSError(),False),(None,False)):
                with mock.patch.object(tool.os,name,side_effect=error) as call:
                    self.assertIs(tool.definitely_absent(42,group),expected)
                    call.assert_called_once_with(42,0)
