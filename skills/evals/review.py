#!/usr/bin/env python3
"""Adapt actual artifacts to the existing skill-creator static viewer; no grading."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--workspace', required=True, type=Path)
    parser.add_argument('--engine', required=True, type=Path)
    args = parser.parse_args()
    root = args.workspace.resolve()
    suite = json.loads((root/'suite.json').read_text())
    view = root/'review-input'
    view.mkdir(exist_ok=True)
    for num, case in enumerate(suite['cases'], 1):
        for arm in ['baseline', 'candidate']:
            for rep in range(1, 4):
                run = root/'iteration-1'/case['id']/arm/f'run-{rep}'
                dest = view/f'{case["id"]}-{arm}-run-{rep}'
                outputs = dest/'outputs'
                outputs.mkdir(parents=True, exist_ok=True)
                grading = json.loads((run/'grading.json').read_text()) if (run/'grading.json').exists() else None
                execution = json.loads((run/'execution.json').read_text()) if (run/'execution.json').exists() else None
                status = grading['status'] if grading else execution['status'] if execution and execution['status'] != 'ungraded' else 'incomplete' if run.exists() else 'not_run'
                state = f'# Actual run result: {status}\n\nEvidence directory: {run}\n\n'
                if grading:
                    state += 'Semantic total: '+str(grading['total'])+'/8. Hard protections override numeric averages.\n\n'
                    state += json.dumps(grading['rubric'], ensure_ascii=False, indent=2)+'\n'
                    adapted = dict(grading)
                    adapted['expectations'] = [dict(e, evidence=e['evidence'] if isinstance(e['evidence'], str) else json.dumps(e['evidence'], ensure_ascii=False)) for e in grading['expectations']]
                    passed = sum(e['passed'] for e in adapted['expectations'])
                    total = len(adapted['expectations'])
                    adapted['summary'] = {'passed': passed, 'failed': total-passed, 'total': total, 'pass_rate': passed/total if total else 0}
                    (dest/'grading.json').write_text(json.dumps(adapted, ensure_ascii=False, indent=2)+'\n')
                else:
                    state += 'No semantic pass is asserted. This row is pending execution or review.\n'
                if execution:
                    state += '\nActual execution:\n'+json.dumps({k: execution.get(k) for k in ['status','exit_code','thread_id','seconds','usage']}, indent=2)+'\n'
                (outputs/'00-status.md').write_text(state)
                (dest/'eval_metadata.json').write_text(json.dumps({'eval_id': num, 'prompt': (run/'prompt.txt').read_text() if (run/'prompt.txt').exists() else case['prompt']}, ensure_ascii=False, indent=2)+'\n')
                if not execution:
                    continue
                for source in (run/'work').rglob('*.md'):
                    rel = source.relative_to(run/'work')
                    if any(p.startswith('.') for p in rel.parts):
                        continue
                    (outputs/('__'.join(rel.parts))).write_bytes(source.read_bytes())
                if (run/'diff.patch').exists():
                    # Authored-source diff only; full runtime diff stays in the evidence directory.
                    sections = (run/'diff.patch').read_text().split('--- before/')
                    diff = ''.join('--- before/'+part for part in sections[1:] if not part.startswith('.mind-forge/'))
                    (outputs/'changes.diff').write_text(diff)
    complete = all((root/'iteration-1'/case['id']/arm/f'run-{rep}'/'grading.json').exists()
                   for case in suite['cases'] for arm in ['baseline', 'candidate'] for rep in range(1, 4))
    title = 'mind-forge complete skill-group primary evaluation — ' + ('full matrix graded' if complete else 'incomplete')
    subprocess.run(['python3', str(args.engine/'eval-viewer/generate_review.py'), str(view), '--skill-name', title, '--static', str(root/'review.html')], check=True)


if __name__ == '__main__':
    main()
