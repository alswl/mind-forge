#!/usr/bin/env python3
"""Report every planned run without upgrading execution success to behavior pass."""
import argparse
import hashlib
from collections import Counter
import json
from pathlib import Path
import statistics


def collect(root, cases):
    rows = []
    for case in cases:
        for arm in ['baseline', 'candidate']:
            for rep in range(1, 4):
                path = root/'iteration-1'/case['id']/arm/f'run-{rep}'
                grading = json.loads((path/'grading.json').read_text()) if (path/'grading.json').exists() else {}
                recovered = grading.get('recovered_compatibility_events', [])
                supplement_path = path/'compatibility-supplement.json'
                if supplement_path.exists():
                    supplement = json.loads(supplement_path.read_text())
                    for name in ['grading', 'transcript']:
                        artifact = path/('grading.json' if name == 'grading' else 'transcript.jsonl')
                        assert hashlib.sha256(artifact.read_bytes()).hexdigest() == supplement[name+'_sha256']
                    recovered = recovered + [e for e in supplement['recovered_compatibility_events'] if e not in recovered]
                execution = json.loads((path/'execution.json').read_text()) if (path/'execution.json').exists() else {}
                if grading:
                    status = grading['status']
                elif execution.get('status') in ['execution_error', 'incomplete']:
                    status = execution['status']
                elif path.exists():
                    status = 'incomplete'
                else:
                    status = 'not_run'
                rows.append({'case': case['id'], 'arm': arm, 'repeat': rep,
                             'status': status, 'score': grading.get('total'),
                             'compatibility_failure': grading.get('compatibility_failure'),
                             'compatibility_observation': grading.get('compatibility_observation'),
                             'recovered_compatibility_events': recovered,
                             'compatibility_supplement': str(supplement_path) if supplement_path.exists() else None,
                             'seconds': execution.get('seconds'), 'usage': execution.get('usage'),
                             'thread_id': execution.get('thread_id'), 'evidence': str(path),
                             'detail': 'pending semantic review' if execution.get('status') == 'ungraded' and not grading else ''})
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--workspace', type=Path, required=True)
    args = ap.parse_args()
    a = args.workspace.resolve()
    suite = json.loads((a/'suite.json').read_text())
    rows = collect(a, suite['cases'])
    batches = {}
    for state_file in sorted(a.glob('parallel-*/state.json')):
        state = json.loads(state_file.read_text())
        for run in state.get('finished', []) + state.get('active', []):
            batches[run['key']] = state_file.parent.name
    for row in rows:
        key = f"{row['case']}/{row['arm']}/run-{row['repeat']}"
        row['execution_batch'] = batches.get(key, 'serial' if row['seconds'] is not None else None)
    stats = {}
    for arm in ['baseline', 'candidate']:
        selected = [r for r in rows if r['arm'] == arm]
        times = [r['seconds'] for r in selected if r['seconds'] is not None]
        scores = [r['score'] for r in selected if r['score'] is not None]
        usage = [u for r in selected for u in (r['usage'] or []) if u]
        stats[arm] = {'counts': dict(Counter(r['status'] for r in selected)),
                      'completed_executions': len(times),
                      'seconds_total': sum(times),
                      'compatibility_failed_runs': sum(r['compatibility_failure'] is True for r in selected),
                      'recovered_compatibility_runs': sum(bool(r['recovered_compatibility_events']) for r in selected),
                      'reported_usage_totals': {key: sum(u.get(key, 0) or 0 for u in usage)
                                                for key in sorted({k for u in usage for k in u})},
                      'time_mean': statistics.mean(times) if times else None,
                      'time_stddev': statistics.stdev(times) if len(times) > 1 else None,
                      'score_mean': statistics.mean(scores) if scores else None,
                      'score_stddev': statistics.stdev(scores) if len(scores) > 1 else None}
    report = {'complete': all(r['status'] in ['pass', 'behavior_fail', 'execution_error'] for r in rows),
              'rows': rows, 'statistics': stats,
              'interpretation': 'No improvement claim until the full fixed paired matrix is graded. Missing samples are not successes; three repetitions do not establish significance.'}
    report['batch_statistics'] = {}
    for batch in sorted({r['execution_batch'] for r in rows if r['execution_batch']}):
        report['batch_statistics'][batch] = {}
        for arm in ['baseline', 'candidate']:
            selected = [r for r in rows if r['execution_batch'] == batch and r['arm'] == arm]
            times = [r['seconds'] for r in selected if r['seconds'] is not None]
            report['batch_statistics'][batch][arm] = {
                'counts': dict(Counter(r['status'] for r in selected)),
                'completed_executions': len(times),
                'seconds_total': sum(times),
                'time_mean': statistics.mean(times) if times else None,
            }
    (a/'results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    lines = ['# Real behavior matrix', '', '| Case | Arm | Repeat | Result | Score /8 | Seconds | Evidence |', '| --- | --- | --- | --- | --- | --- | --- |']
    for row in rows:
        path = Path(row['evidence']).relative_to(a)
        lines.append(f"| {row['case']} | {row['arm']} | {row['repeat']} | {row['status']} | {row['score'] if row['score'] is not None else '—'} | {row['seconds'] if row['seconds'] is not None else '—'} | {path} |")
    lines += ['', report['interpretation'], '', '```json', json.dumps(stats, indent=2), '```']
    lines += ['', '## Execution batches', '',
              'The first 14 primary runs were serial. Subsequent approved batches use at most three concurrent frozen runners, each with a distinct case/arm/repetition directory. Inputs and model settings are unchanged, but concurrency and temporal batches differ; pooled latency is not a controlled speed comparison.', '',
              '```json', json.dumps(report['batch_statistics'], indent=2), '```']
    lines += ['', 'Usage fields are reported by the actual CLI. Cached input is a subset of input; do not add them together. No billing or unique-context-token estimate is inferred.', '',
              '## Compatibility observations', '',
              'Recovered legacy-command errors remain separate from task behavior failures. All observations are retained in results.json.', '']
    for row in rows:
        if row['compatibility_failure'] or row['recovered_compatibility_events']:
            lines.append(f"- {row['case']} {row['arm']} run-{row['repeat']}: {row['compatibility_observation']}")
    (a/'results.md').write_text('\n'.join(lines)+'\n')
    print(json.dumps(stats, indent=2))


if __name__ == '__main__':
    main()
