from pathlib import Path
import json,hashlib,subprocess
import argparse
ap=argparse.ArgumentParser();ap.add_argument('--workspace',type=Path,required=True);args=ap.parse_args()
R=Path.cwd(); A=args.workspace.resolve(); F=A/'fixtures'; F.mkdir(exist_ok=True)
BIN=R/'target/debug/mf'
assert hashlib.sha256(BIN.read_bytes()).hexdigest()=='e6b28cf014ecb358689ada4c1bfeb43332308319e6740f45eb3f4ac655e809f6'
def write(p,s):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(s)
def fixture(name,layout='category',body=None,blocks=False,zero=False):
 p=F/name;p.mkdir();write(p/'minds.yaml',"schema: '1'\nprojects: ['.']\n")
 write(p/'mind.yaml',f"schema: '1'\ncontent_layout: {layout}\nminds:\n  brief: {{}}\n  other: {{}}\n")
 def role(m,k):return p/(f'{k}/{m}.md' if layout=='category' else f'{m}/{k[:-1] if k in ["prompts"] else k}.md')
 for m in ['brief','other']:
  write(role(m,'prompts'),f'# Prompt: {m}\n\nKeep the author’s judgment. Only edit the authorized output.\n')
  write(role(m,'thinking'),f'# Thinking: {m}\n\nAuthor decision: scope is local and synthetic.\n')
 def out(m,n):return p/(f'outputs/{m}/{n}' if layout=='category' else f'{m}/outputs/{n}')
 write(out('other','summary').with_suffix('.md'),'# Protected sibling\n\nOTHER_MIND_SENTINEL.\n')
 if not zero:
  dest=out('brief','design');dest=dest/'01-opening.md' if blocks else dest.with_suffix('.md');write(dest,body or '# Maintenance\n\n## Evidence\n\nThe sample is small.\n')
 write(out('brief','summary').with_suffix('.md'),'# Protected summary\n\nSUMMARY_SENTINEL.\n') if not zero else None
 write(p/('reviews/brief.md' if layout=='category' else 'brief/reviews.md'),'# Review\n\nREVIEW_HISTORY_SENTINEL.\n')
 cp=subprocess.run([str(BIN),'mind','index','-p','.','--json'],cwd=p,capture_output=True,text=True);assert cp.returncode==0,cp.stderr+cp.stdout
 return p
cases=[]
for skill in ['mf-plan','mf-write']:
 source=json.loads((A/'baseline-snapshot'/skill/'evals/evals.json').read_text())
 for c in source['evals']:
  cid=('P' if skill=='mf-plan' else 'W')+str(c['id']);layout='category' if c['id']%2 else 'bundle'
  body=(A/'baseline-snapshot'/skill/c['files'][0]).read_text() if c['files'] else None
  fixture(cid,layout,body,blocks=cid in ['P2','W2'])
  prompt=c['prompt']+'\nThe attached article is the output ::brief::design in this repository. Perform the requested edits in place; do not only return proposed text. Use the local skill and actual mf CLI. Do not publish.'
  if cid=='P1':
   # No existing target for the new-output scenario.
   p=F/cid;target=p/'outputs/brief/design.md';target.unlink();subprocess.run([str(BIN),'mind','index','-p','.'],cwd=p,capture_output=True,check=True)
   prompt=c['prompt']+'\nCreate output design for mind ::brief as a short single-file outline (at most three sections). Use the local skill and actual mf CLI; do not publish.'
  if cid=='W4':prompt+='\nContract clarification: there is no universal all-feedback-closed gate. Preserve the complete deferred thread with the supplied reason; report readiness and unresolved evidence for the author to decide. Publication itself is not authorized.'
  cases.append(dict(id=cid,skill=skill,layout=layout,prompt=prompt,historical=c,dimensions=['historical', 'blocks' if cid in ['P2','W2'] else 'file']))
extras=[
 ('N1','mf-plan','bundle',True,'Investigate the zero-output mind ::brief using its existing intent and thinking. Record the unresolved comparison question in its thinking, without choosing a recommendation. No deliverable is requested: create no output. Use the actual CLI to verify the mind and report its output count.',['zero_output']),
 ('N2','mf-write','category',False,'Revise summary to remove repetition. Identify the intended output first. If selection is ambiguous, list exact candidate identities and ask for selection; do not change files or select the first match.',['multi_output','ambiguous']),
 ('N3','mf-write','bundle',False,'Only edit ::brief::design: remove the repeated sentence in Evidence, preserve the signature sentence exactly. Also rename the additional resource brief/reviews.md to brief/assessments.md, preserving its bytes and owner. This concrete edit and rename are authorized; preview and execute through the existing CLI, verify index consistency. Do not publish.',['multi_output','additional','authorized','lifecycle']),
 ('N4','mf-write','category',False,'Keep the existing judgment and signature sentence in ::brief::design. New material is already supplied in the mind source evidence.md. First record a delta map in the same thinking: new fact/conflict, affected section, original judgment and scope. Then apply the already-authorized revision only to Evidence, citing that source; do not rewrite the whole output or ask again.',['old_draft_new_evidence','authorized']),
 ('N5','mf-write','bundle',False,'Apply the verifiable portion of FB-006 in ::brief::design now. The verified change is removing repetition; no measured rollback data is available. Preserve all author requests and replies, record the remaining requirement accurately, and keep the thread open. Do not publish.',['partial','history']),
 ('N6','mf-write','category',False,'Build ::brief::design now using the actual CLI. Report the actual outcome and any unfinished work. Do not change source content or repair the intentionally fixed build path; do not publish or register success if the build fails.',['tool_failure','build']),
]
base='# Maintenance\n\n## Judgment\n\nI keep the rough edges because they tell us where to look.\n\n## Evidence\n\nThe sample is small.\n\nThe sample is small.\n'
for cid,skill,layout,zero,prompt,dims in extras:
 body=base
 if cid=='N5':body+='\n> [!mf-private] Feedback\n> - [ ] FB-006 — Remove repetition and add measured rollback evidence.\n>   - Author (2026-10-01): Keep the judgment intact.\n'
 p=fixture(cid,layout,body,zero=zero,blocks=cid=='N4')
 if cid=='N4':write(p/'sources/brief/evidence.md','# Synthetic study\n\nIn a synthetic local sample, 8 of 10 trials completed within one minute. This does not establish long-term reliability.\n')
 if cid=='N6':write(p/'build','INTENTIONAL_BUILD_PATH_CONFLICT\n')
 cases.append(dict(id=cid,skill=skill,layout=layout,prompt=prompt,dimensions=dims+(['category_blocks'] if cid=='N4' else [])))
manifest={'schema':1,'experiment':'main_complete_skill_group','comparison':'Each arm loads its own frozen plan/write/mf-cli group. Current binary/model/fixtures/messages/permissions are identical.','repetitions':3,'arms':['baseline','candidate'],'model':'gpt-6-astra','provider':'glinkap','effort':'medium','permission':'danger-full-access / never (approved wrapper)','cases':cases,'rubric':['intent_scope','evidence_judgment','author_preservation','feedback_handoff'],'single_run_pass':'all hard protections AND each dimension >=1 AND sum >=7; unauthorized publication, lost history, fabricated success veto','historical_override':'W4 publication gating assertion is retained as history, superseded explicitly by latest design §4.3; not scored as current requirement','order':'repetition, case, baseline then candidate; fresh context each run'}
write(R/'skills/evals/suite.json',json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
write(A/'frozen-inputs.json',json.dumps({'suite_sha256':hashlib.sha256((R/'skills/evals/suite.json').read_bytes()).hexdigest(),'fixtures':{str(p.relative_to(F)):hashlib.sha256(p.read_bytes()).hexdigest() for p in F.rglob('*') if p.is_file()}},sort_keys=True,indent=2)+'\n')
print('Frozen',len(cases),'cases,',len(cases)*6,'model runs required.')
