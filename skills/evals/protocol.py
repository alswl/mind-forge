#!/usr/bin/env python3
"""Verify actual session identity and immutable tool/skill bytes for completed runs."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--workspace', required=True, type=Path)
    args = parser.parse_args()
    root = args.workspace.resolve()
    manifest = json.loads((root/'run-manifest.json').read_text())
    for run in sorted((root/'iteration-1').glob('*/*/run-*')):
        if not (run/'execution.json').exists():
            continue
        actual = json.loads((run/'session-identity.json').read_text())
        expected = {'model': manifest['model'], 'model_provider': manifest['provider'],
                    'effort': manifest['effort'], 'approval_policy': manifest['approval_policy'],
                    'sandbox_policy': {'type': manifest['permission']}, 'cwd': str(run/'work')}
        checks = [{'text': 'actual_'+key, 'passed': actual.get(key) == value,
                   'evidence': {'expected': value, 'actual': actual.get(key)}} for key, value in expected.items()]
        arm = run.parent.name
        paths = [(root/'bin/mf', run/'work/.mind-forge/tools/mf')]
        paths += [(source, run/'work/.skills'/source.relative_to(root/f'{arm}-snapshot'))
                  for source in (root/f'{arm}-snapshot').rglob('*') if source.is_file()]
        for original, copied in paths:
            old = hashlib.sha256(original.read_bytes()).hexdigest()
            new = hashlib.sha256(copied.read_bytes()).hexdigest() if copied.is_file() else None
            checks.append({'text': 'immutable_'+str(copied.relative_to(run/'work')),
                           'passed': old == new, 'evidence': {'expected': old, 'actual': new}})
        result = {'all_passed': all(c['passed'] for c in checks), 'expectations': checks}
        (run/'protocol-checks.json').write_text(json.dumps(result, indent=2)+'\n')
        print(run.relative_to(root), 'protocol_pass' if result['all_passed'] else 'protocol_fail')


if __name__ == '__main__':
    main()
