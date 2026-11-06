"""Observe existing benchmark renders at their stdout syscall; never judge budgets."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def descendants(pid):
    found = [pid]
    for current in found:
        try:
            children = Path(f'/proc/{current}/task/{current}/children').read_text()
        except (FileNotFoundError, ProcessLookupError):
            continue
        found.extend(int(child) for child in children.split() if int(child) not in found)
    return found[1:]


def capture(pid, destination):
    process = Path(f'/proc/{pid}')
    try:
        words = process.joinpath('cmdline').read_bytes().split(b'\0')
        words = [word for word in words if word]
        if len(words) != 3 or words[1:] != [b'statusline', b'render']:
            return None
        call = process.joinpath('syscall').read_text().split()
        if len(call) < 2 or call[0] != '1' or int(call[1], 0) != 1:
            return None
        contents = {name: process.joinpath(name).read_text() for name in ['status', 'smaps_rollup', 'smaps']}
    except (FileNotFoundError, ProcessLookupError):
        return None
    for name, text in contents.items():
        destination.joinpath(f'render-{pid}-{name}.txt').write_text(text)
    fields = {}
    for line in contents['status'].splitlines():
        if line.startswith(('VmRSS:', 'VmHWM:', 'RssAnon:', 'RssFile:', 'RssShmem:')):
            key, value = line.split(':', 1)
            fields[key] = value.strip()
    return {'pid': pid, 'atSyscall': 'write(stdout)', 'memory': fields}


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def main():
    measurer, application, source, target = map(Path, sys.argv[1:])
    target.mkdir(parents=True, exist_ok=True)
    command = ['strace', '-f', '-qq', '--kill-on-exit', '-o', str(target / 'syscalls.log'),
               '-e', 'trace=execve,clone,clone3,fork,vfork,write,exit_group',
               '-e', 'inject=write:delay_enter=150ms:when=1+', str(measurer),
               'bench-statusline', '--root', str(source), '--bin', str(application),
               '--runs', '1', '--out', str(target / 'observed-benchmark.json')]
    env = os.environ.copy()
    env['XTASK_MEASURER'] = str(measurer)
    snapshots = {}
    with target.joinpath('coordinator.log').open('w') as log:
        process = subprocess.Popen(command, env=env, stdout=log, stderr=log)
        deadline = time.monotonic() + 40
        try:
            while process.poll() is None and time.monotonic() < deadline:
                for pid in descendants(process.pid):
                    if pid not in snapshots:
                        snapshot = capture(pid, target)
                        if snapshot is not None:
                            snapshots[pid] = snapshot
                time.sleep(0.01)
            if process.poll() is None:
                raise TimeoutError('Observation exceeded 40 seconds')
            if process.returncode != 0:
                raise RuntimeError(f'Trace exited {process.returncode}; see coordinator.log')
        finally:
            stop(process)
    if len(snapshots) != 5:
        raise RuntimeError(f'Expected all 3 warmups + 1 warm + 1 cold render, captured {len(snapshots)}')
    report = {'observationOnly': True, 'delayedSyscall': 'every write per process, 150ms',
              'budgetAssertion': 'unchanged separate CI step; not evaluated by this probe', 'renders': list(snapshots.values())}
    target.joinpath('render-memory.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
