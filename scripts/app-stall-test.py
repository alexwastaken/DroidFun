#!/usr/bin/env python3
"""Pause only the selected app; always resume it, then measure frame recovery."""
import argparse
import json
import os
import pathlib
import signal
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('--telemetry-dir', required=True)
p.add_argument('--output', required=True)
p.add_argument('--hold', type=int, default=75)
a = p.parse_args()
if not 10 <= a.hold <= 600:
    p.error('--hold must be between 10 and 600 seconds')
def interrupted(signum, frame):
    raise KeyboardInterrupt
signal.signal(signal.SIGTERM, interrupted)
path = pathlib.Path(a.telemetry_dir) / 'telemetry.json'
before = json.loads(path.read_text())
pid = before['pid']
name = subprocess.check_output(['ps', '-p', str(pid), '-o', 'comm='], text=True).strip()
assert pathlib.Path(name).name == 'deskbot' and time.time() - before['at'] < 10
assert before['playback'] == 'pinned' and before['displayOn']
assert any(w['current'] and w['kind'] in ['weather','stock'] for w in before['widgets'])
assert before['frameCrc'] == before['link']['lastFrameCrc'], 'Wait for the pinned frame to be acknowledged'
report = {'startedAt': time.time(), 'holdSeconds': a.hold, 'before': before, 'physicalFallbackReview': 'pending'}
try:
    os.kill(pid, signal.SIGSTOP)
    time.sleep(a.hold)
finally:
    os.kill(pid, signal.SIGCONT)
    report['resumedAt'] = time.time()
until = time.monotonic() + 12
while time.monotonic() < until:
    after = json.loads(path.read_text())
    if after['pid'] != pid:
        raise RuntimeError('The app process changed during the stall test')
    if after['at'] >= report['resumedAt'] + 5:
        break
    time.sleep(0.25)
report['after'] = after
report['frameResent'] = after['link']['frameAcks'] > before['link']['frameAcks']
report['expectedFrameCrc'] = before['frameCrc']
report['crcMatches'] = after['link']['lastFrameCrc'] == before['frameCrc']
report['result'] = 'static_frame_recovery_passed' if report['frameResent'] and report['crcMatches'] and after['connection']['state'] == 'connected' else 'static_frame_recovery_failed'
report['boundary'] = 'Frame resend and board CRC are measured; physical fallback appearance/timing needs user observation.'
target = pathlib.Path(a.output)
target.parent.mkdir(parents=True, exist_ok=True)
target.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k:report[k] for k in ['result','frameResent','crcMatches','holdSeconds']}))
