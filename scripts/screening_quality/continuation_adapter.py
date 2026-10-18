"""Reviewed continuation authority rules, exposed without changing adoption."""
import copy
import hashlib
from pathlib import Path
from .assembly import assemble, bind_private, check, reader


def body(value):
    return {k: v for k, v in value.items() if k not in ("adopted", "origin")}


def same_binding(actual, expected, recorded_root):
    def relative(name):
        path = Path(name)
        if path.is_absolute():
            path = path.relative_to(recorded_root)
        check(not path.is_absolute() and ".." not in path.parts, "unsafe bound path")
        return str(path)
    check(relative(actual["path"]) == relative(expected["path"]) and
          all(actual[k] == expected[k] for k in ("sha256", "bytes", "mode")),
          "raw authority path/hash/size/mode differs")


def recorded_root(freeze):
    case = freeze["blocks"][0]["cases"][0]
    relative = Path(case["output"])
    absolute = Path(case["command"][8])
    check(not relative.is_absolute() and absolute.is_absolute() and
          absolute.parts[-len(relative.parts):] == relative.parts, "recorded root mismatch")
    return absolute.parents[len(relative.parts) - 1]


def resolve(wrapper, old, archive, original_global, ledger, epoch, completion,
            load, bind_new, protocol, wrapper_binding):
    """All JSON objects must already be verified via their wrapper bindings.

    bind_new(path) binds a bounded0600 per-case file at the EPOCH-frozen path,
    never at a path chosen by the new global report. load verifies each binding.
    protocol is the independently source-pinned reviewed continuation module;
    only its pure build_ledger/common_assets/slots helpers are called.
    """
    for key, expected_sha in (
            ("freezeSha256", wrapper["epochFreeze"]["sha256"]),
            ("wrapperSha256", wrapper_binding["sha256"]),
            ("adoptionLedgerSha256", wrapper["adoptionLedger"]["sha256"])):
        check(isinstance(expected_sha, str) and len(expected_sha) == 64 and
              completion.get(key) == expected_sha, "global provenance binding differs: " + key)
    check(wrapper["schemaVersion"] == 1 and wrapper["kind"] == "prospective-pretranscription-continuation-v1",
          "wrong wrapper")
    check(wrapper["originalFreeze"]["sha256"] == protocol.OLD_FREEZE_SHA and
          wrapper["originalArchive"]["sha256"] == protocol.OLD_ARCHIVE_SHA, "unfrozen old authority")
    check(wrapper["adoptedPrefixCases"] == 185 and wrapper["newCases"] == 215 and
          wrapper["rerunFromSlot"] == 186 and wrapper["v3AdoptionAllowed"] is False and
          wrapper["qualified"] is False and wrapper["L"] is None, "continuation policy differs")
    check(wrapper["originalCompletion"] == archive["bindings"]["completion"], "old global authority differs")
    expected = protocol.build_ledger(old, archive)
    expected["continuationId"] = wrapper["continuationId"]
    check(ledger == expected, "adoption ledger is not independently canonical")
    protocol.common_assets(old, epoch)
    check(epoch["sourceRevision"] == wrapper["sourceRevision"], "new source differs")
    check(completion["continuationId"] == wrapper["continuationId"] and
          completion["oldFreezeSha256"] == wrapper["originalFreeze"]["sha256"] and
          completion["completed"] is True and completion["error"] is None and
          completion["rowCount"] == completion["plannedRows"] == 800 and
          completion["adoptedCases"] == 185 and completion["adoptedRows"] == 370 and
          completion["newCasesRequested"] == completion["newLaunchAttempts"] == 215 and
          completion["newRowsRequested"] == 430 and completion["powerAssertionEnded"] is True and
          completion["qualified"] is False and completion["L"] is None, "incomplete or wrong global epoch")
    check(original_global["completed"] is False and len(original_global["blocks"]) == 4,
          "old global altered")
    check([(b["name"], b["variant"], len(b["cases"])) for b in original_global["blocks"]] ==
          [(b["name"], b["variant"], 100) for b in old["blocks"]], "old global block order differs")
    old_flat = [c for b in original_global["blocks"] for c in b["cases"]]
    new_flat = [c for b in completion["blocks"] for c in b["cases"]]
    check(len(old_flat) == len(new_flat) == 400 and
          [(b["name"], b["variant"]) for b in completion["blocks"]] ==
          [(b["name"], b["variant"]) for b in epoch["blocks"]], "global slot/block mismatch")
    root = recorded_root(old)
    check(recorded_root(epoch) == root and epoch["outputDirectory"] != old["outputDirectory"],
          "new namespace/root mismatch")
    authorities = []
    parameter_values = {v: load(binding, "authority") for v, binding in epoch["parameters"].items()}
    for i, (block, case_index, case) in enumerate(protocol.slots(epoch)):
        entry = new_flat[i]
        for key in ("output", "metadata", "log", "completion"):
            path = Path(case[key])
            check(not path.is_absolute() and ".." not in path.parts and
                  path.parent == Path(epoch["outputDirectory"]), "new path outside frozen namespace")
        check(case["command"][8] == str(root / case["output"]), "new command/path differs")
        check(entry["id"] == case["id"], "global case reordered")
        if i < 185:
            ref = ledger["entries"][i]
            original = load(ref["completion"], "completion")
            check(original == old_flat[i] and body(entry) == original, "adopted source body changed")
            check(ref["processIdentity"] == dict(epoch=protocol.OLD_FREEZE_SHA,
                  wrapperPid=original["launch"]["wrapperPid"], probePid=original["actualProbePid"],
                  startedUtc=original["launch"]["startedUtc"]), "adopted process identity differs")
            completion_binding = ref["completion"]
            raw_expected = ref["rawFiles"]
            revision = old["sourceRevision"]
            epoch_id = protocol.OLD_FREEZE_SHA
        else:
            completion_binding = bind_new(case["completion"])
            original = load(completion_binding, "completion")
            check(original == entry, "new per-case/global completion differs")
            raw_expected = [dict(raw, path=case[key]) for raw, key in
                            zip(original["rawFiles"], ("output", "metadata", "log"))]
            revision = epoch["sourceRevision"]
            epoch_id = wrapper["continuationId"]
        check(original["status"] == "ok" and original["error"] is None and
              original["launch"]["exitCode"] == 0 and original["launch"]["terminationConfirmed"] is True and
              original["launch"]["launchError"] is None and
              all(original[k]["source"] == "AC Power" and original[k]["error"] is None
                  for k in ("beforePower", "afterPower")), "source owner/power case failed")
        check(len(original["rawFiles"]) == len(raw_expected) == 3, "source raw incomplete")
        for actual, expected_raw in zip(original["rawFiles"], raw_expected):
            same_binding(actual, expected_raw, root)
        origin = dict(epoch=epoch_id, sourceRevision=revision, completionPath=completion_binding["path"])
        check(entry["origin"] == origin and entry["adopted"] is (i < 185), "reported origin differs from frozen authority")
        metadata = dict(sourceRevision=revision, processId=original["actualProbePid"],
                        parameterDocumentSha256=epoch["parameters"][block["variant"]]["sha256"],
                        parameters=parameter_values[block["variant"]], model=dict(id="large-v3-turbo-q5_0", sha256=epoch["models"][0]["sha256"]),
                        fixtureSha256=None)
        # Fixture fields are added from independently bound full100 manifest below.
        metadata.pop("fixtureSha256")
        metadata.update(pretranscriptionCandidate=block["variant"] == "PretransCandidate",
                        warmupRuns=1, measuredRunsRequested=1)
        authorities.append(dict(block=block["name"], variant=block["variant"], id=case["id"],
                                adopted=i < 185, origin=origin, completion=completion_binding,
                                rawFiles=copy.deepcopy(original["rawFiles"]), metadata=metadata))
    return authorities


def prepare(root, wrapper_binding, completion_binding, protocol):
    """Actual archive entry point, only after all400 cases are terminal+valid."""
    root = Path(root).resolve(strict=True)
    read = reader(root)
    def authority(binding):
        return read(binding, "authority")
    wrapper = authority(wrapper_binding)
    old = authority(wrapper["originalFreeze"])
    archive = authority(wrapper["originalArchive"])
    original_global = authority(wrapper["originalCompletion"])
    ledger = authority(wrapper["adoptionLedger"])
    epoch = authority(wrapper["epochFreeze"])
    completion = read(completion_binding, "completion")
    # Bind the actual global report at the epoch-frozen path as well.
    check(Path(completion_binding["path"]) == Path(epoch["outputDirectory"]) / "completion.json",
          "new global path not frozen")
    manifest = authority(epoch["manifest"])
    check(epoch["manifest"]["sha256"] == "df432b44c48a39ebe290c6bd20d9ba8b800fd2211cdd898c86a9b22a54edf030",
          "full100 manifest changed")
    def bind_new(name):
        return bind_private(root,name)
    authorities = resolve(wrapper, old, archive, original_global, ledger, epoch, completion,
                          read, bind_new, protocol, wrapper_binding)
    for authority, case in zip(authorities, manifest["cases"] * 4):
        check(authority["id"] == case["id"], "manifest slot differs")
        authority["metadata"].update(fixtureSha256=case["audio"]["sha256"],
                                     inputFrames=case["audio"]["frames"], sampleRate=16000, channels=1,
                                     language=case["language"],
                                     expectedOutcome="Empty" if case["reference"] == "" else "Local")
    results, provenance = assemble(manifest, completion, authorities, read)
    provenance["assemblySourceBindings"] = []
    for name in (Path(__file__), Path(__file__).with_name("assembly.py"), Path(__file__).with_name("profiles.py"), Path(protocol.__file__)):
        data = name.read_bytes()
        provenance["assemblySourceBindings"].append(dict(path=str(name), bytes=len(data),
            sha256=hashlib.sha256(data).hexdigest()))
    provenance["authorityBindings"] = dict(wrapper=wrapper_binding, completion=completion_binding,
                                         **{k: wrapper[k] for k in ("originalFreeze", "originalArchive",
                                            "originalCompletion", "adoptionLedger", "epochFreeze")})
    return manifest, results, provenance
