#!/usr/bin/env python3
"""Synthetic, offline triage. Never points mf at a real knowledge repository."""
import hashlib, json, os, pathlib, platform, shutil, subprocess, tempfile
OUT = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(tempfile.mkdtemp(prefix='mf-triage-'))
MF = shutil.which('mf')
rows=[]
def run(label,args,cwd=None):
    p=subprocess.run(args,cwd=cwd or ROOT,text=True,capture_output=True)
    row=dict(id=label,argv=args,cwd=str(cwd or ROOT),exit=p.returncode,stdout=p.stdout,stderr=p.stderr)
    rows.append(row); print(label,p.returncode, p.stdout[:120].replace('\n',' '),p.stderr[:100].replace('\n',' '),flush=True)
    return p

def mf(label,*args,cwd=None): return run(label,[MF,*args],cwd)
def write(p,s): p.parent.mkdir(parents=True,exist_ok=True);p.write_text(s)
def snap(label,p):
    data={str(f.relative_to(p)):f.read_text(errors='replace') for f in p.rglob('*') if f.is_file() and '.git' not in f.parts}
    (OUT/(label+'.json')).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
def note(label,**kw): rows.append(dict(id=label,**kw))
mf('version','version','--json')
note('environment',platform=platform.platform(),binary=MF,sha256=hashlib.sha256(pathlib.Path(MF).read_bytes()).hexdigest(),fixture=str(ROOT))
mf('init','init',str(ROOT))
mf('project','project','new','projects/probe')
P=ROOT/'projects/probe'
# All configuration is synthetic. No endpoints, credentials or external tools.
write(P/'mind.yaml',"schema: '1'\nproject:\n  name: probe\nbuild:\n  output_dir: outputs\n  format: md\npublish:\n  targets:\n    - name: local\n      type: local\n      enabled: true\n      config:\n        path: "+str(ROOT/'published')+"\n")
write(ROOT/'minds.yaml',"schema: '1'\nprojects:\n  - projects/probe\nsource:\n  backend: legacy\n")
def m(label,*args):return mf(label,*args,cwd=P)
m('file-create','article','new','single','--file','--json')
m('dir-create','article','new','chunked','--json')
m('help-update','publish','update','--help')
for selector in ['single','docs/single.md','chunked','docs/chunked','docs/chunked/','docs/chunked/01-opening.md']:
    for dry in [True,False]:
        m('update-'+selector+('-dry' if dry else ''),'publish','update',selector,'--target','local','--set','status=published','--json',*(['--dry-run'] if dry else []))
snap('publish-update',P)
# Current build vs overwrite vs stale build are separate contracts.
write(P/'docs/single.md','# Single\n\nVERSION_ONE\n')
m('build-v1','build','single')
m('publish-v1','publish','run','single','--target','local','--json')
write(P/'docs/single.md','# Single\n\nVERSION_TWO\n')
m('build-v2','build','single')
m('republish-text','publish','run','single','--target','local')
m('republish-json','publish','run','single','--target','local','--json')
note('after-refusal',destination=(ROOT/'published/single.md').read_text(),build=(P/'outputs/single.md').read_text())
m('republish-force','publish','run','single','--target','local','--force','--json')
note('after-force',destination=(ROOT/'published/single.md').read_text())
write(P/'docs/single.md','# Single\n\nVERSION_THREE_NOT_BUILT\n')
m('force-without-build','publish','run','single','--target','local','--force','--json')
note('stale-build-observation',destination=(ROOT/'published/single.md').read_text())
m('build-dir','build','chunked')
m('publish-dir','publish','run','docs/chunked','--target','local','--json')
snap('publish-run',P)
# Conversion leaves record identity observable.
m('convert','article','convert','--to-directory','--json')
snap('after-convert',P)
# Adjacent private callouts: preserve public paragraphs.
m('private-create','article','new','private-probe','--file')
write(P/'docs/private-probe.md','# Public title\n\nPUBLIC_BEFORE\n\n> [!mf-private] A\n> SECRET_A\n\n> [!mf-private] B\n> SECRET_B\n\nPUBLIC_AFTER\n')
m('private-build','build','private-probe')
note('private-artifact',content=(P/'outputs/private-probe.md').read_text())
# Index reconciliation with existing and missing local source files.
write(P/'sources/file/alpha.md','synthetic source alpha\n')
m('source-index-adopt','source','index','--json')
m('source-index-stable','source','index')
m('source-index-stable-json','source','index','--json')
snap('source-before-missing',P)
(P/'sources/file/alpha.md').unlink()
m('source-index-missing','source','index')
m('source-index-missing-json','source','index','--json')
m('source-list-missing','source','list','--json')
snap('source-after-missing',P)
# Archive source lifecycle checks.
write(P/'sources/file/beta.md','synthetic beta\n')
m('source-register','source','new','sources/file/beta.md','--register-only','--no-index','--json')
(P/'sources/renamed').mkdir()
m('source-rename','source','rename','beta','sources/renamed/gamma.md','--json')
m('source-remove-keep','source','remove','gamma','--keep-file','--yes','--force','--json')
note('keep-file',exists=(P/'sources/renamed/gamma.md').exists())
m('source-reregister','source','new','sources/renamed/gamma.md','--register-only','--no-index')
m('source-remove-default','source','remove','gamma','--yes','--force','--json')
note('remove-file',exists=(P/'sources/renamed/gamma.md').exists())
# Unknown term and supported two-step alternative.
m('correction-unknown','term','correction','add','Widget','Widgit','Widget','--json')
m('term-new','term','new','Widget','--definition','Synthetic example','--json')
m('correction-add','term','correction','add','Widget','Widgit','Widget','--json')
# Existing article plus binding, inbound link, rename/move.
m('rename-create','article','new','old-name','--file')
write(P/'prompts/old-name.md','---\narticle: docs/old-name.md\n---\nSynthetic intent\n')
write(P/'thinking/old-name.md','Synthetic ledger\n')
write(P/'docs/referrer.md','# Referrer\n\n[Target](old-name.md)\n')
m('index-bindings','article','index','--json')
m('rename-dry','article','rename','docs/old-name.md','new-name','--dry-run','--json')
m('rename','article','rename','docs/old-name.md','new-name','--json')
snap('after-rename',P)
mf('target-project','project','new','projects/destination')
m('move','article','move','docs/new-name.md','--to-project',str(ROOT/'projects/destination'),'--json')
snap('after-move',ROOT/'projects')
# Four drift classes, then missing directories.
write(P/'prompts/orphan.md','---\narticle: docs/absent.md\n---\nOrphan\n')
write(P/'docs/unindexed.md','# Unindexed\n')
(P/'docs/referrer.md').unlink()
m('drift-lint','project','lint','--json')
m('drift-article-index-preview','article','index','--dry-run','--json')
m('dot-index','article','index','--project','.','--json')
m('dot-lint','project','lint','--project','.','--json')
m('dot-build','build','chunked','--project','.','--dry-run','--json')
for name in ['sources','assets']:
    shutil.rmtree(P/name,ignore_errors=True)
m('missing-dirs','project','lint','--json')
m('fix-dirs','project','lint','--fix','--json')
note('fixed-directory-files',sources=list(str(x.relative_to(P)) for x in (P/'sources').rglob('*')),assets=list(str(x.relative_to(P)) for x in (P/'assets').rglob('*')))
run('git-init',['git','init',str(ROOT)])
run('git-untracked',['git','ls-files','--others','--exclude-standard'],P)
# Block selector archive check.
write(P/'docs/chunked/02-middle.md','## Middle\n')
write(P/'docs/chunked/03-end.md','## End\n')
for anchor in ['01-opening.md','01-opening','opening','absent']:
    m('block-anchor-'+anchor,'article','block','move','docs/chunked','03-end','--after',anchor,'--dry-run','--json')
# Isolated lint repair without unrelated orphan errors.
mf('empty-project','project','new','projects/empty')
E=ROOT/'projects/empty'
for name in ['sources','assets']:
    shutil.rmtree(E/name,ignore_errors=True)
mf('empty-lint-before','project','lint','--project','.','--json',cwd=E)
mf('empty-lint-fix','project','lint','--project','.','--fix','--json',cwd=E)
mf('empty-lint-after','project','lint','--project','.','--json',cwd=E)
run('empty-git-files',['git','ls-files','--others','--exclude-standard'],E)
snap('final-fixture',ROOT)
(OUT/'runs.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
print('evidence',OUT,'fixture',ROOT)
