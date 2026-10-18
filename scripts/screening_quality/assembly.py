"""Shared bounded quality-input assembly, not an evaluator or new run authority.

assemble() accepts 400 slot authorities resolved from the selected adapter's
verified frozen descriptors, never from self-reported origins in the global
completion. Each authority binds the original per-case completion and its three raw
files. The caller binds manifest/completion/authorities themselves. Neither profile
issues a semantic anchor verdict or a qualification from these copied strings.

No quality.evaluate here: remote Python 3.14 is not canonical Unicode 13.
"""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from .profiles import CONTINUATION, FULL_PREFIX

RUNS = (("warmup", "warmup", True), ("measured", "measured-001", False))


def check(condition, message):
    if not condition:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def assemble(manifest, completion, authorities, load, profile=CONTINUATION):
    """Return eight strict 3-field result arrays plus content-free provenance.

    load(binding, role) must verify actual regular-file mode/size/SHA before
    returning parsed JSON (or None for a fully hashed log). No output is published
    until every slot validates. Original hypotheses are copied byte-for-byte as
    JSON string values: no strip, normalization, reference matching or repair.
    """
    check(manifest.get("schemaVersion") == 1 and manifest.get("unicodeVersion") == "13.0.0", "manifest contract differs")
    cases = manifest["cases"]
    check(len(cases) == 100 and len({c["id"] for c in cases}) == 100, "expected 100 unique manifest cases")
    check(profile in (CONTINUATION,FULL_PREFIX), "explicit known assembly profile required")
    profile.completion(completion)
    check(len(authorities) == 400 and len(completion["blocks"]) == 4, "expected 400 authorities/cases")
    check(tuple(b["variant"] for b in completion["blocks"]) == profile.variants, "ABBA differs")
    check(len({b["name"] for b in completion["blocks"]}) == 4, "duplicate block")
    results, slots, seen_paths = {}, [], set()
    for bi, block in enumerate(completion["blocks"]):
        check(len(block["cases"]) == 100, "incomplete block")
        for phase, _, _ in RUNS:
            results[(block["name"], phase)] = []
        for ci, (case, entry) in enumerate(zip(cases, block["cases"])):
            index = bi * 100 + ci
            authority = authorities[index]
            check((authority["block"], authority["variant"], authority["id"]) ==
                  (block["name"], block["variant"], case["id"]) and entry["id"] == case["id"], "slot identity/order differs")
            slot_origin = profile.slot(completion,authority,entry,index)
            check(entry["status"] == "ok" and entry["error"] is None, "case failed or unavailable")
            raw = authority["rawFiles"]
            check(entry["rawFiles"] == raw and len(raw) == 3, "raw authority differs")
            for binding in [authority["completion"]] + raw:
                check(binding["mode"] == "0o600" and binding["path"] not in seen_paths, "nonprivate or reused raw path")
                seen_paths.add(binding["path"])
            source = load(authority["completion"], "completion")
            profile.source(source,entry)
            check(len(entry["rows"]) == 2, "missing validated rows")
            raw_results = load(raw[0], "results")
            metadata = load(raw[1], "metadata")
            load(raw[2], "log")
            check(type(raw_results) is list and len(raw_results) == 2, "raw result row count differs")
            # These metadata fields come from the frozen slot, not from reports.
            for key, expected in authority.get("metadata", {}).items():
                check(metadata.get(key) == expected, "metadata binding differs: " + key)
            for ri, (phase, run, warmup) in enumerate(RUNS):
                row, raw_row = entry["rows"][ri], raw_results[ri]
                check(row["id"] == case["id"] and row["runId"] == run and row["warmup"] is warmup
                      and row["status"] == "ok" and row.get("validated") is True, "unvalidated/reordered logical row")
                check(type(raw_row) is dict and set(raw_row) == {"id", "status", "warmup", "text"}
                      and raw_row["id"] == run and raw_row["warmup"] is warmup and raw_row["status"] == "ok", "raw run identity differs")
                text = raw_row["text"]
                check(isinstance(text, str) and len(text.encode("utf-8")) <= 256 * 1024, "invalid/oversized result text")
                results[(block["name"], phase)].append(dict(id=case["id"], status="ok", text=text))
                slots.append(dict(slot=index + 1, block=block["name"], variant=block["variant"],
                                  caseIndex=ci + 1, id=case["id"], phase=phase, rawRunId=run,
                                  **copy.deepcopy(slot_origin),
                                  completion=copy.deepcopy(authority["completion"]), rawFiles=copy.deepcopy(raw),
                                  sourceRawRowCanonicalSha256=digest(raw_row),
                                  referenceUtf8Sha256=hashlib.sha256(case["reference"].encode("utf-8")).hexdigest()))
    provenance = dict(schemaVersion=1, kind=profile.provenance_kind, unicodeVersion="13.0.0",
                      canonicalEvaluationPerformed=False, qualified=False, L=None,
                      manifestCanonicalSha256=digest(manifest), completionCanonicalSha256=digest(completion),
                      authoritiesCanonicalSha256=digest(authorities), **profile.counts(slots), slots=slots,
                      inputs=[dict(block=block, phase=phase, rows=len(rows), canonicalSha256=digest(rows))
                              for (block, phase), rows in results.items()])
    return results, provenance


def reader(root):
    """Reuse existing FD-bound, no-follow bounded verification; no raw output."""
    root = Path(root).resolve(strict=True)
    spec = importlib.util.spec_from_file_location("assembly_common", root / "scripts/run-paced-remainder.py")
    common = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(common)
    fingerprints = {}

    def load(binding, role):
        check(role in ("authority","source") or binding.get("mode") == "0o600", "private binding required")
        local = dict(binding)
        path = Path(local["path"])
        if path.is_absolute():
            local["path"] = str(path.relative_to(root))
        maximum = common.MAX_METADATA if role == "metadata" else common.MAX_TEXT
        data = common.verify_binding(root, local, fingerprints, maximum, read=role not in ("log","source"))
        return None if role in ("log","source") else common.parse_json(data)
    return load


def publish_inputs(root, output, manifest, results, provenance):
    """Exclusive private output, only AFTER assemble succeeded for all 800 rows.

    Publishes inputs, never fake canonical reports on a non-Unicode13 host.
    An interruption can leave an incomplete new directory; no overwrite/retry.
    Final input-bindings.json is the publication completion marker.
    """
    import os
    root = Path(root).resolve(strict=True)
    output = Path(output)
    check(output.is_absolute() and not output.is_symlink(), "absolute nonsymlink output required")
    # Resolve parent aliases and traversal before containment and use that
    # canonical path for all writes. In particular /var -> /private/var is
    # valid, while artifacts/../outside or a parent symlink escape is not.
    output = output.resolve(strict=False)
    artifacts = root / "artifacts"
    check(artifacts.is_dir() and not artifacts.is_symlink() and artifacts in output.parents,
          "private artifacts output required")
    spec = importlib.util.spec_from_file_location("assembly_publish", root / "scripts/run-paced-remainder.py")
    common = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(common)
    payloads = {"manifest.json": manifest, "origin.json": provenance}
    for bi, ((block, phase), rows) in enumerate(results.items()):
        check(len(rows) == 100, "incomplete output input")
        payloads["B%d-%s.results.json" % (bi // 2 + 1, phase)] = rows
    check(len(results) == 8 and len(payloads) == 10, "exactly eight phase/block inputs required")
    encoded = {name: (json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n").encode("utf-8")
               for name, value in payloads.items()}
    check(all(len(data) <= common.MAX_JSON for data in encoded.values()), "assembled input exceeds canonical CLI bound")
    output.mkdir(mode=0o700)
    bindings = []
    for name, data in encoded.items():
        path = output / name
        common.write_private_bytes(path, data)
        bindings.append(dict(path=str(path.relative_to(root)), bytes=len(data), mode="0o600",
                             sha256=hashlib.sha256(data).hexdigest()))
    common.write_private(output / "input-bindings.json", dict(kind="complete-eight-quality-inputs", bindings=bindings,
                         canonicalEvaluationPerformed=False, qualified=False, L=None))


def bind_private(root, name):
    """Capture a bounded private per-case completion at an authorized path."""
    import os
    import stat
    root = Path(root).resolve(strict=True)
    relative = Path(name)
    check(not relative.is_absolute() and ".." not in relative.parts, "unsafe new completion path")
    path = root / relative
    check(root in path.resolve().parents, "completion resolves outside repository")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "rb") as stream:
        info = os.fstat(stream.fileno())
        check(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o600 and
              info.st_size <= 16*1024*1024, "new case completion not private bounded regular")
        data = stream.read(info.st_size + 1)
        after = os.fstat(stream.fileno())
        check(len(data) == info.st_size and
              (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns) ==
              (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns),
              "new completion changed during bind")
    return dict(path=name, mode="0o600", bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
