"""Prepare the synthetic report arms using the frozen CLI and a local pnpm store."""
import json
import shutil
import subprocess


def add_report_cases(root, binary, cases, fixture, body, store):
    definitions = json.loads((root / 'skills/mf-report/evals/evals.json').read_text())['evals']
    starter = root / 'pkg/services/report/starter'
    registry = root / 'pkg/services/report/testdata/registry'
    for definition in definitions:
        number = definition['id']
        cid = f'R{number}'
        layout = 'bundle' if number == 2 else 'category'
        project = fixture(cid, layout, body)
        if number in (1, 2):
            subprocess.run([binary, 'report', 'init', 'weekly', '-p', '.',
                            '--renderer', 'builtin/trend'], cwd=project, check=True, capture_output=True)
            report = project / 'templates/reports/weekly'
            if number == 2:
                renderer = project / '.mind-forge/report/renderers/weekly'
                shutil.copytree(registry / 'renderers/weekly', renderer)
                (renderer / 'renderer.yaml').write_text(
                    (renderer / 'renderer.yaml').read_text().replace('repo/delivery, ', ''))
                (renderer / 'Report.tsx').write_text(
                    'export default function Weekly({ data }) { return <p>{data.completed} requests</p> }\n')
                shutil.copyfile(renderer / 'example.json', report / 'data.json')
                spec = report / 'report.yaml'
                spec.write_text(spec.read_text().replace('builtin/trend', 'repo/weekly'))
            for name in ('package.json', 'pnpm-lock.yaml'):
                shutil.copyfile(starter / name, report / name)
            subprocess.run(['corepack', 'pnpm@10.28.2', 'install', '--frozen-lockfile',
                            '--offline', '--store-dir', store], cwd=report, check=True)
        cases.append(dict(id=cid, skill='mf-write' if number == 3 else 'mf-report',
                          layout=layout, prompt=definition['prompt'], historical=definition,
                          dimensions=['report', ['builtin', 'component', 'markdown'][number - 1]]))
