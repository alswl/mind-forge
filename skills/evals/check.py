#!/usr/bin/env python3
"""Extract hard protection evidence. Semantic grading remains a separate review."""
import argparse
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re


def renderer_renders_delivery(source):
    source = re.sub(r'//[^\n]*|/\*.*?\*/', '', source, flags=re.S)
    if 'data-report-component' in source:
        return False
    lookup = r'''\b(?:const|let|var)\s+([\w$]+)\s*=\s*components\s*\.\s*get\(\s*(["'])repo/delivery\2\s*\)'''
    for match in re.finditer(lookup, source):
        component = re.escape(match[1])
        invocation = rf'<{component}(?=[\s/>])|\bcreateElement\(\s*{component}(?=\s*[,\)])'
        if re.search(invocation, source[match.end():]):
            return True
    return False


class DeliveryMarkup(HTMLParser):
    void_tags = set('area base br col embed hr img input link meta param source track wbr'.split())

    def __init__(self):
        super().__init__()
        self.elements = []
        self.components = []

    def handle_starttag(self, tag, attrs):
        if tag in self.void_tags:
            return
        attrs = dict(attrs)
        parent_visible, text = self.elements[-1][1:] if self.elements else (True, None)
        visible = parent_visible and tag not in ('script', 'style', 'template') and 'hidden' not in attrs
        if visible and text is None and attrs.get('data-report-component') == 'repo/delivery':
            text = []
            self.components.append(text)
        self.elements.append((tag, visible, text))

    def handle_endtag(self, tag):
        for index in range(len(self.elements) - 1, -1, -1):
            if self.elements[index][0] == tag:
                del self.elements[index:]
                break

    def handle_data(self, data):
        if self.elements:
            _, visible, text = self.elements[-1]
            if visible and text is not None:
                text.append(data)


def report_contains_delivery(html):
    parser = DeliveryMarkup()
    parser.feed(html)
    parser.close()
    for component in parser.components:
        text = ' '.join(' '.join(component).split())
        if re.search(r'\bDelivery completion\b', text) and re.search(r'\b7 requests\b', text):
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--workspace', type=Path, required=True)
    args = ap.parse_args()
    a = args.workspace.resolve()
    suite = json.loads((a/'suite.json').read_text())
    for case in suite['cases']:
        cid = case['id']
        layout = case['layout']
        output = 'outputs/brief/design' if layout == 'category' else 'brief/outputs/design'
        prompt = 'prompts/brief.md' if layout == 'category' else 'brief/prompt.md'
        thinking = 'thinking/brief.md' if layout == 'category' else 'brief/thinking.md'
        allowed = {prompt, thinking, 'mind-index.yaml'}
        if cid == 'N1':
            allowed = {thinking, 'mind-index.yaml'}
        if cid in ['N2', 'N6']:
            allowed = set()
        if cid == 'N3':
            allowed |= {'brief/reviews.md', 'brief/assessments.md'}
        for run in sorted((a/'iteration-1'/cid).glob('*/run-*')):
            if not (run/'execution.json').exists():
                continue
            changes = json.loads((run/'changes.json').read_text())
            out_changes = [p for p in changes if p not in allowed and
                           not p.startswith('.mind-forge/') and
                           not (cid not in ['N1', 'N2', 'N6'] and
                                (p == output+'.md' or p.startswith(output+'/')))]
            report_root = 'templates/reports/weekly/'
            report_outputs = {report_root + 'outputs/report.' + view + '.html' for view in ('chart', 'table')}
            report_catalog = report_root + 'outputs/catalog.html'
            if cid in ['R1', 'R2']:
                report_allowed = report_outputs.copy()
                if cid == 'R2':
                    report_allowed |= {report_catalog, '.mind-forge/report/renderers/weekly/Report.tsx',
                                       '.mind-forge/report/renderers/weekly/renderer.yaml'}
                out_changes = [p for p in changes if p not in report_allowed and
                               not (cid == 'R2' and p.startswith('.mind-forge/report/components/delivery/'))]
            evidence = []
            def check(name, ok, why):
                evidence.append({'text': name, 'passed': bool(ok), 'evidence': why})
            check('authorized_write_set', not out_changes,
                  {'unexpected_paths': out_changes, 'all_changes': changes})
            fixture = a/'fixtures'/cid
            work = run/'work'
            after = json.loads((run/'after.json').read_text())
            before = json.loads((run/'before.json').read_text())
            protected = [p for p in before if p.startswith(('outputs/other/', 'other/', 'prompts/other.', 'thinking/other.'))
                         or p.endswith('/summary.md') or p.startswith('sources/')]
            mismatch = [p for p in protected if after.get(p) != before[p]]
            check('sibling_and_evidence_hashes', not mismatch,
                  {'protected': {p: before[p] for p in protected}, 'mismatched': mismatch})
            authored = [p for p in fixture.rglob('*.md') if str(p.relative_to(fixture)) == output+'.md' or str(p.relative_to(fixture)).startswith(output+'/')]
            old = '\n'.join(p.read_text() for p in authored)
            new_files = [work/p.relative_to(fixture) for p in authored] + [work/prompt]
            new = '\n'.join(p.read_text() for p in new_files if p.is_file())
            historic = [line for line in old.splitlines() if re.match(r'>\s+(?:- \[[ x]\] FB-|\s*- (?:Author|Agent) \()', line)]
            normalize = lambda line: re.sub(r'\[[ x]\]', '[STATE]', line).strip()
            # Deferral may remove blockquote syntax but must preserve actual wording.
            fragments = [re.sub(r'^>\s*', '', normalize(line)) for line in historic]
            normalized_new = normalize(new)
            missing = [f for f in fragments if f not in normalized_new]
            positions = [normalized_new.find(f) for f in fragments]
            ordered = positions == sorted(positions) and all(v >= 0 for v in positions)
            check('immutable_feedback_history', not missing and ordered, {'required_fragments': fragments, 'missing': missing, 'ordered': ordered})
            sig = 'I keep the rough edges because they tell us where to look.'
            check('signature_sentence', sig not in old or sig in new, {'text': sig, 'protected_utf8_sha256': hashlib.sha256(sig.encode()).hexdigest(), 'exact_match': sig in new} if sig in old else 'not applicable')
            events = []
            for line in (run/'transcript.jsonl').read_text().splitlines():
                try: events.append(json.loads(line))
                except ValueError: pass
            commands = [e['item'] for e in events if e.get('type') == 'item.completed' and e.get('item', {}).get('type') == 'command_execution']
            real = [i for i in commands if re.search(r'(?:/mf|\bmf)\s+(mind|output|article|build|additional|lifecycle|report)', i.get('command', ''))]
            check('real_cli_calls', bool(real), [{'id': i['id'], 'command': i['command'], 'exit_code': i.get('exit_code')} for i in real])
            forbidden = [i['id'] for i in commands if re.search(r'\bpublish\s+(run|update)\b', i.get('command', ''))]
            check('no_unauthorized_publication', not forbidden, {'event_ids': forbidden})
            if cid == 'N5':
                check('partial_stays_open', '[ ] FB-006' in new and '[x] FB-006' not in new, 'FB-006 in edited source')
            if cid == 'N6':
                failed = [i['id'] for i in real if 'build' in i['command'] and ('"status":"error"' in i.get('aggregated_output', '') or i.get('exit_code', 0) != 0)]
                check('observed_build_failure', bool(failed), {'event_ids': failed})
            if cid == 'P1':
                check('new_short_output_is_single_file', (work/(output+'.md')).is_file() and not (work/output).is_dir(), output+'.md')
            if cid in ['R1', 'R2']:
                expected_id = 'builtin/trend' if cid == 'R1' else 'repo/weekly'
                exports = sorted(report_outputs)
                check('dual_exports', all((work/p).is_file() for p in exports), {'artifacts': exports})
                check('observed_renderer', any(expected_id in i.get('aggregated_output', '') for i in real), expected_id)
                if cid == 'R1':
                    expected_values = ('0 requests', 'Not reported')
                else:
                    expected_values = ('7', 'requests')
                    catalog = work/report_catalog
                    check('catalog_preview', catalog.is_file() and 'repo/delivery' in catalog.read_text(), report_catalog)
                    renderer = '.mind-forge/report/renderers/weekly/Report.tsx'
                    source = work/renderer
                    check('renderer_renders_delivery', source.is_file() and renderer_renders_delivery(source.read_text()), renderer)
                    rendered = [p for p in exports if (work/p).is_file() and report_contains_delivery((work/p).read_text())]
                    check('rendered_delivery_component', rendered == exports, {'matched_exports': rendered})
                check('matching_report_values', all((work/p).is_file() and
                      all(v in (work/p).read_text() for v in expected_values) for p in exports), expected_values)
                check('offline_content_policy', all((work/p).is_file() and
                      "default-src 'none'" in (work/p).read_text()
                      for p in exports), {'artifacts': exports, 'browser_checks': 'see transcript'})
            if cid == 'R3':
                writes = [p for p in changes if p.startswith(('templates/reports/', '.mind-forge/report/'))]
                report_calls = [i['id'] for i in real if re.search(r'(?:/mf|\bmf)\s+report', i.get('command', ''))]
                check('markdown_workflow', not writes and not report_calls and new.count('The sample is small.') == 1,
                      {'report_writes': writes, 'report_calls': report_calls})
            (run/'hard-checks.json').write_text(json.dumps({'expectations': evidence, 'all_passed': all(i['passed'] for i in evidence), 'semantic_status': 'not_graded'}, ensure_ascii=False, indent=2)+'\n')
            print(cid, run.parent.name, run.name, 'hard_pass' if all(i['passed'] for i in evidence) else 'hard_fail', flush=True)


if __name__ == '__main__':
    main()
