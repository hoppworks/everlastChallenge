"""Two explicit mode/origin contracts; never infer a history from a report."""
import copy
import json


def check(condition, message):
    if not condition:
        raise ValueError(message)


class _Continuation:
    variants = ("PretransReference", "PretransCandidate", "PretransCandidate", "PretransReference")
    provenance_kind = "continuation-quality-assembly"

    def completion(self, value):
        check(value.get("kind") == "pretranscription-continuation-execution" and
              value.get("completed") is True and value.get("error") is None, "continuation not fully complete")

    def slot(self, completion, authority, entry, index):
        # Preserve the reviewed 185/215 adoption boundary and original epochs.
        check(type(entry["adopted"]) is bool and entry["adopted"] is authority["adopted"] is (index < 185),
              "adoption boundary differs")
        check(entry["origin"] == authority["origin"], "stale/unbound origin")
        epoch = completion["oldFreezeSha256"] if entry["adopted"] else completion["continuationId"]
        check(entry["origin"]["epoch"] == epoch and
              authority["completion"]["path"] == entry["origin"]["completionPath"], "origin epoch/path differs")
        return dict(adopted=entry["adopted"],origin=copy.deepcopy(entry["origin"]))

    def source(self, source, entry):
        projected = {key:value for key,value in entry.items() if key not in ("adopted","origin")}
        check(source == projected or source == entry, "original completion changed")

    def counts(self, slots):
        adopted = sum(slot["adopted"] for slot in slots[::2])
        return dict(adoptedCases=adopted,newCases=400-adopted)


class _FullPrefix:
    variants = ("FullPrefixReference", "FullPrefixCandidate", "FullPrefixCandidate", "FullPrefixReference")
    provenance_kind = "full-prefix-quality-assembly-v1"

    def completion(self, value):
        check(value.get("kind") == "full-prefix-screening-execution-v1" and value.get("completed") is True and
              value.get("error") is None and type(value.get("rowCount")) is int and value["rowCount"] == 800 and
              value.get("qualified") is False and value.get("L") is None and value.get("powerAssertionEnded") is True,
              "full-prefix screen not fully complete")

    def slot(self, completion, authority, entry, index):
        check("adopted" not in entry and "origin" not in entry and "adopted" not in authority,
              "full-prefix cases cannot inherit continuation adoption")
        check(type(entry["globalSlot"]) is int and entry["globalSlot"] == index+1 and
              entry["blockName"] == authority["block"] and entry["variant"] == authority["variant"] and
              entry["screeningId"] == completion["screeningId"] and entry["freezeSha256"] == completion["freezeSha256"],
              "fresh full-prefix case identity differs")
        origin = dict(freezeSha256=completion["freezeSha256"],screeningId=completion["screeningId"],
                      sourceRevision=completion["sourceRevision"],completionPath=authority["completion"]["path"])
        check(authority["origin"] == origin, "unbound full-prefix origin")
        return dict(origin=copy.deepcopy(origin))

    def source(self, source, entry):
        check(json.dumps(source,sort_keys=True,allow_nan=False) == json.dumps(entry,sort_keys=True,allow_nan=False),
              "full-prefix case/global completion changed")

    def counts(self, slots):
        return dict(freshCases=400)


CONTINUATION = _Continuation()
FULL_PREFIX = _FullPrefix()
