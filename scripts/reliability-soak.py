#!/usr/bin/env python3
"""Local, redacted collector. Does not open serial or send model requests.

With --faults, briefly interrupt only Deskbot's HTTP reader and host serial
connection. Physical removal and operating-system suspend are separate tests.
"""
import argparse
import datetime
import json
import os
import pathlib
import re
import signal
import statistics
import subprocess
import time


def utc(at):
    return datetime.datetime.fromtimestamp(at, datetime.timezone.utc).isoformat()


def atomic(path, value):
    temp = path.with_suffix('.tmp')
    temp.write_text(json.dumps(value, indent=2) + '\n')
    temp.chmod(0o600)
    temp.replace(path)


def cpu_seconds(value):
    parts = [float(v) for v in value.split(':')]
    return sum(v * (60 ** i) for i, v in enumerate(reversed(parts)))


def processes(pid):
    rows = {}
    text = subprocess.check_output(['ps', '-axo', 'pid=,ppid=,time=,rss=,comm='], text=True)
    for line in text.splitlines():
        fields = line.split(None, 4)
        if len(fields) == 5:
            p, parent, cpu, rss, name = fields
            rows[int(p)] = {'pid': int(p), 'parent': int(parent), 'cpuSeconds': cpu_seconds(cpu), 'rssMiB': int(rss) / 1024, 'name': pathlib.Path(name).name}
    if pid not in rows or rows[pid]['name'] != 'deskbot':
        return []
    owned = {pid}
    while True:
        more = {p for p, r in rows.items() if r['parent'] in owned}
        if more <= owned:
            break
        owned |= more
    # macOS XPC webview services have launchd as their parent. Launch Services
    # attributes their display names to Deskbot; include only its exact names
    # after independently matching the single app process PID.
    inventory = subprocess.check_output(['lsappinfo', 'list'], text=True)
    blocks = re.split(r'(?m)^\d+\) ', inventory)
    app_found = any(b.startswith('"Deskbot" ') and re.search(r'pid = ' + str(pid) + r'\b', b) for b in blocks)
    xpc = set()
    if app_found:
        for block in blocks:
            if any(block.startswith('"' + name + '" ') for name in ['Deskbot Networking', 'Deskbot Graphics and Media', 'Deskbot Web Content']):
                match = re.search(r'pid = (\d+)', block)
                if match and int(match[1]) in rows:
                    xpc.add(int(match[1]))
    owned |= xpc
    codex = {p for p in owned if rows[p]['name'] == 'codex'}
    while True:
        more = {p for p in owned if rows[p]['parent'] in codex}
        if more <= codex:
            break
        codex |= more
    for p in owned:
        rows[p]['uiProcess'] = p == pid or p in xpc
        rows[p]['includedInAppTotals'] = p not in codex
    return [rows[p] for p in sorted(owned)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run-dir', required=True)
    parser.add_argument('--telemetry-dir', help='Existing app measurement directory (preflight only)')
    parser.add_argument('--seconds', type=int, default=86400)
    parser.add_argument('--faults', action='store_true')
    parser.add_argument('--static', action='store_true')
    parser.add_argument('--keep-awake', action='store_true')
    args = parser.parse_args()
    directory = pathlib.Path(args.run_dir).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    telemetry_dir = pathlib.Path(args.telemetry_dir).resolve() if args.telemetry_dir else directory
    if args.faults and telemetry_dir != directory:
        parser.error('Fault runs must use their own app telemetry directory')
    if args.seconds <= 0:
        parser.error('--seconds must be positive')
    if (directory / 'run.json').exists():
        parser.error('Use a new run directory; prior evidence is preserved')
    lock = (directory / 'collector.lock').open('w')
    import fcntl
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    for name in ['network-offline', 'link-interruption']:
        if (directory / name).exists():
            parser.error('Fault markers already exist; inspect the interrupted run first')
    stop = False
    def interrupted(signum, frame):
        nonlocal stop
        stop = True
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    started, mono = time.time(), time.monotonic()
    run = {'formatVersion': 2, 'collectorPid': os.getpid(), 'startedAt': started, 'startedUtc': utc(started), 'expectedEndAt': started + args.seconds, 'expectedEndUtc': utc(started + args.seconds), 'durationSeconds': args.seconds, 'status': 'running', 'controlledFaults': args.faults, 'keepAwake': args.keep_awake, 'memoryScope': 'Deskbot, its named WebKit XPC services and sampled helper descendants, excluding only the optional Codex child subtree. RSS sum can double count shared pages; short-lived helpers may fall between samples.'}
    atomic(directory / 'run.json', run)
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-i', '-w', str(os.getpid())]) if args.keep_awake else None
    schedule = [('network-offline', 1200, 2400), ('link-interruption', 3600, 3620), ('network-offline', 43200, 44400), ('link-interruption', 64800, 64820)] if args.faults else []
    active = set()
    fault_events, issues, rss, cpu = [], [], [], []
    app_rss, app_cpu, closed_rss = [], [], []
    static_valid = True
    static_crc = None
    process_prev = {}
    samples, healthy, gaps, maximum_pending, frame_acks = 0, 0, 0, 0, 0
    widget_visits = {}
    last_seen = None
    last_pid = None
    last_frame_count = 0
    last_sample_mono, last_sample_wall = mono, started
    first_main_cpu, last_main_cpu = None, None
    cpu_observed_seconds = 0
    log = (directory / 'samples.jsonl').open('a', buffering=1)
    log_path = directory / 'samples.jsonl'
    log_path.chmod(0o600)
    try:
        while not stop:
            now, steady = time.time(), time.monotonic()
            elapsed = now - started
            desired = {name for name, begin, end in schedule if begin <= elapsed < end}
            for name in active - desired:
                (directory / name).unlink(missing_ok=True)
                fault_events.append({'at': now, 'fault': name, 'active': False})
            for name in desired - active:
                (directory / name).write_text('Owned by this Deskbot reliability collector\n')
                fault_events.append({'at': now, 'fault': name, 'active': True})
            active = desired
            gap = now - last_sample_wall > 15 or abs((now - last_sample_wall) - (steady - last_sample_mono)) > 5
            if gap:
                gaps += 1
            try:
                telemetry = json.loads((telemetry_dir / 'telemetry.json').read_text())
                age = now - telemetry['at']
                rows = processes(telemetry['pid'])
                state = telemetry['connection']['state']
                ack_age = now - telemetry['link']['lastAckAt']
                snapshot_ack_age = telemetry['at'] - telemetry['link']['lastAckAt']
                valid = bool(rows) and age <= 15
                link_clears = [e['at'] for e in fault_events if e['fault'] == 'link-interruption' and not e['active']]
                expected_recovery = bool(link_clears) and now - link_clears[-1] <= 5
                expected_interruption = 'link-interruption' in active or expected_recovery
                # ACKs arrive every two seconds; the telemetry file is sampled
                # every five. Compare timestamps from the same snapshot to
                # avoid falsely adding the collector's sampling delay.
                good = valid and state == 'connected' and telemetry['connection']['displayHealth'] == 'ready' and snapshot_ack_age <= 4
                issue = None if good or expected_interruption else ('app_or_telemetry_missing' if not valid else 'robot_not_healthy')
                if issue != last_seen:
                    if issue:
                        issues.append({'at': now, 'issue': issue, 'connectionState': state})
                    elif last_seen:
                        issues.append({'at': now, 'issue': 'recovered'})
                    last_seen = issue
                if rows:
                    main_row = next(r for r in rows if r['pid'] == telemetry['pid'])
                    rss.append(main_row['rssMiB'])
                    app_rss.append(sum(r['rssMiB'] for r in rows if r['includedInAppTotals']))
                    if not telemetry['controlsVisible']:
                        closed_rss.append(app_rss[-1])
                    prior = process_prev.get(telemetry['pid'])
                    if prior and not gap:
                        delta = main_row['cpuSeconds'] - prior['cpuSeconds']
                        duration = steady - last_sample_mono
                        if delta >= 0 and duration > 0:
                            cpu.append(delta / duration * 100)
                            cpu_observed_seconds += duration
                            ui_delta = sum(max(0, r['cpuSeconds'] - process_prev.get(r['pid'], r)['cpuSeconds']) for r in rows if r['includedInAppTotals'])
                            app_cpu.append(ui_delta / duration * 100)
                    process_prev = {r['pid']: r for r in rows}
                    if first_main_cpu is None:
                        first_main_cpu = main_row['cpuSeconds']
                    last_main_cpu = main_row['cpuSeconds']
                count = telemetry['link']['frameAcks']
                if telemetry['pid'] == last_pid and count >= last_frame_count:
                    frame_acks += count - last_frame_count
                elif last_pid is not None:
                    frame_acks += count
                last_pid, last_frame_count = telemetry['pid'], count
                current = next((w['kind'] for w in telemetry['widgets'] if w['current']), 'none')
                if args.static:
                    if static_crc is None:
                        static_crc = telemetry['frameCrc']
                    static_valid = static_valid and not telemetry['controlsVisible'] and current == 'face' and telemetry['frameCrc'] == static_crc and telemetry['playback'] == 'pinned'
                widget_visits[current] = widget_visits.get(current, 0) + 1
                maximum_pending = max(maximum_pending, telemetry['pendingRequests'] or 0)
                healthy += bool(good)
                entry = {'at': now, 'telemetryAgeSeconds': age, 'ackAgeSeconds': ack_age, 'snapshotAckAgeSeconds':snapshot_ack_age,'gap': gap, 'healthy': good, 'expectedInterruption': expected_interruption, 'expectedRecoveryGrace':expected_recovery, 'processes': rows, 'telemetry': telemetry}
            except (OSError, ValueError, KeyError, TypeError, StopIteration, subprocess.SubprocessError) as error:
                entry = {'at': now, 'healthy': False, 'gap': gap, 'collectorError': type(error).__name__}
                if last_seen != 'collector_read_failed':
                    issues.append({'at': now, 'issue': 'collector_read_failed'})
                    last_seen = 'collector_read_failed'
            log.write(json.dumps(entry, separators=(',', ':')) + '\n')
            samples += 1
            cpu_mean = statistics.mean(cpu) if cpu else None
            current_report = {**run, 'lastSampleAt': now, 'elapsedSeconds': elapsed, 'samples': samples, 'healthySamples': healthy, 'samplingGaps': gaps, 'mainRssMiB': {'min': min(rss) if rss else None, 'max': max(rss) if rss else None, 'latest': rss[-1] if rss else None, 'earlyMedian': statistics.median(rss[:min(120, len(rss))]) if rss else None, 'recentMedian': statistics.median(rss[-120:]) if rss else None}, 'appRssMiB': {'max':max(app_rss) if app_rss else None,'latest':app_rss[-1] if app_rss else None,'earlyMedian':statistics.median(app_rss[:120]) if app_rss else None,'recentMedian':statistics.median(app_rss[-120:]) if app_rss else None},'closedPanelRssMaxMiB':max(closed_rss) if closed_rss else None,'appCpuPercentMean':statistics.mean(app_cpu) if app_cpu else None,'staticConditionsValid':static_valid if args.static else None,'mainCpuPercentMean': cpu_mean, 'cpuObservedSeconds': cpu_observed_seconds, 'maximumPendingRequests': maximum_pending, 'frameAcks': frame_acks, 'widgetSamples': widget_visits, 'faultEvents': fault_events, 'issues': issues, 'currentIssue': last_seen, 'result': 'running', 'physicalTests': 'Separate user-observed checks; frame ACKs do not prove OLED pixel appearance.'}
            atomic(directory / 'progress.json', current_report)
            last_sample_wall, last_sample_mono = now, steady
            if elapsed >= args.seconds:
                break
            time.sleep(min(5, max(0, args.seconds - elapsed)))
    finally:
        for name in active:
            (directory / name).unlink(missing_ok=True)
        if awake:
            awake.terminate()
            awake.wait(timeout=3)
        log.close()
    finished = time.time()
    completed = not stop and finished - started >= args.seconds
    result = 'interrupted' if not completed else 'needs_review' if issues or gaps else 'observation_complete'
    if args.static and completed and cpu_mean is not None:
        result = 'static_app_targets_passed' if args.seconds >= 300 and cpu_observed_seconds >= 295 and static_valid and statistics.mean(app_cpu) < 1 and max(app_rss) * 1024 * 1024 < 100_000_000 and not issues and not gaps else 'static_app_targets_failed'
    final = {**current_report, 'status': 'finished' if completed else 'interrupted', 'endedAt': finished, 'endedUtc': utc(finished), 'result': result, 'faultCleanup': True}
    atomic(directory / 'report.json', final)
    atomic(directory / 'progress.json', final)
    atomic(directory / 'run.json', {**run, 'status': final['status'], 'endedAt': finished})
    print(json.dumps({'result': result, 'samples': samples, 'report': str(directory / 'report.json')}))


if __name__ == '__main__':
    main()
