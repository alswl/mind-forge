#!/usr/bin/env python3
import hashlib,json,pathlib,subprocess
D=pathlib.Path(__file__).resolve().parent
prior=json.loads((D/'runs.json').read_text()); env=next(r for r in prior if r['id']=='environment')
R=pathlib.Path(env['fixture']); P=R/'projects/probe'; MF=env['binary']; rows=[]
def run(id,*args):
 p=subprocess.run([MF,*args],cwd=P,capture_output=True,text=True)
 rows.append(dict(id=id,argv=[MF,*args],cwd=str(P),exit=p.returncode,stdout=p.stdout,stderr=p.stderr))
def digest():
 return {str(p.relative_to(R)):hashlib.sha256(p.read_bytes()).hexdigest() for p in R.rglob('*') if p.is_file() and '.git' not in p.parts}
run('heldback-term','term','new','Device','--definition','Synthetic device')
run('heldback-correction','term','correction','add','Device','机器','Device')
p=P/'docs/heldback.md';p.write_text('# Probe\n\n机器\n')
run('heldback-normal','term','lint','docs/heldback.md','--project','.','--json')
run('heldback-exclude','term','lint','docs/heldback.md','--project','.','--exclude-original','机器','--json')
before=digest()
run('update-dry-fail','publish','update','docs/chunked','--target','local','--set','status=published','--dry-run','--json')
rows.append(dict(id='dry-no-write',unchanged=before==digest()))
# Test links and unindexed files without other errors or mutation.
(P/'prompts/orphan.md').unlink()
run('reconcile-before-drift','article','index','--json')
(P/'docs/link-probe.md').write_text('# Link\n\n[Absent](absent.md)\n')
run('index-link','article','index','--json')
(P/'docs/not-indexed.md').write_text('# Not indexed\n')
run('link-and-unindexed-lint','project','lint','--project','.','--json')
run('link-article-lint','article','lint','--project','.','--json')
run('unindexed-preview','article','index','--dry-run','--json')
(D/'supplement.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
for x in rows: print(x['id'],x.get('exit',x.get('unchanged')),x.get('stdout','')[:200])
