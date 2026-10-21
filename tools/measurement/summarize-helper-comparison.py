#!/usr/bin/env python3
"""Recompute medians from the published historical comparison rows; no ASR run."""
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys

source = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    'docs/benchmarks/helper-comparison-2026-10-20.json'
)
data = json.loads(source.read_text())
rows = data['rows']
if len(rows) != 30 or len({row['slot'] for row in rows}) != 30:
    raise ValueError('Expected 30 distinct measurement slots')
summary = []
for case in data['cases']:
    groups = {}
    transcripts = set()
    for side in ('everlast', 'voicely'):
        selected = [r for r in rows if r['caseId'] == case['id'] and r['side'] == side]
        if sorted(r['repetition'] for r in selected) != [1, 2, 3, 4, 5]:
            raise ValueError('Expected five repetitions per case and side')
        for row in selected:
            if (row['status'] != 'ok' or row['exitCode'] != 0 or row['timeout']
                    or not row['processGroupAbsent'] or row['descendantsAfterLeaderExit']
                    or not math.isfinite(row['wallMs']) or row['wallMs'] <= 0):
                raise ValueError('Incomplete or invalid measurement')
            transcripts.add(row['transcriptSha256'])
        times = [r['wallMs'] for r in selected]
        groups[side] = {'medianMs': statistics.median(times), 'minMs': min(times), 'maxMs': max(times)}
    if len(transcripts) != 1:
        raise ValueError('Transcripts differ within or between sides')
    summary.append({'caseId': case['id'], **groups,
                    'reductionPercent': 100 * (1 - groups['everlast']['medianMs'] / groups['voicely']['medianMs']),
                    'ratio': groups['voicely']['medianMs'] / groups['everlast']['medianMs']})
print(json.dumps({'inputSha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'cases': summary}, indent=2))
