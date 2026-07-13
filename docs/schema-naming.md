# Schema Noun Naming

This note records the naming evaluation for the workspace schema entities and the
decisions taken when `prompts/` and `thinking/` were promoted to first-class,
queryable schema (spec `065-prompts-thinking-schema`). It exists so a future
rename is a deliberate, already-reasoned change rather than a fresh debate.

## Current entities

| Directory | Entity / command | Holds |
|-----------|------------------|-------|
| `docs/` | `Article` | The user-readable deliverable |
| `sources/` | `Source` | Evidence and provenance |
| `assets/` | `Asset` | Binary/media attachments |
| `prompts/` | `Prompt` | Control plane: objective, mode, constraints, evaluation criteria, dated decisions |
| `thinking/` | `Thinking` | Working ledger: comparisons, assumptions, feedback ledger, decisions, blockers |

Note the existing precedent that a **directory name need not equal its entity
name** (`docs/` ↔ `Article`). A future rename can therefore change an entity/command
noun while leaving the on-disk directory name (and existing repos) untouched.

## Evaluation

Well-named and stable: `Source`, `Asset`, `mode` (`editorial`/`research`/
`decision-research`), the `article` binding field, and `binding_status`
(`bound`/`orphan`). No change intended.

Two weaker names, and one minor one, were identified:

- **`prompt` is overloaded three ways in this CLI.** It already means (1) the
  model-facing render prompt emitted by `mf render` (see Constitution I,
  "Prompt-emitting commands, such as `mf render`"), and (2) the TTY confirmation
  prompt. The `prompts/` store is a third meaning: an article's creation
  *brief* / intent spec. For an AI-native CLI, forcing the agent to
  disambiguate one word across three concepts is a real ergonomic cost, and the
  store's actual content is closer to a "brief" or "intent" than a "prompt".
- **`thinking` is a gerund, not an entity noun.** `mf article list` reads
  naturally; `mf thinking list` less so. The workflow skills already call this
  store the "working ledger" / "thinking ledger" / "feedback ledger" — i.e. its
  true noun is **ledger**.
- **`duplicate_binding` is slightly imprecise.** The condition is two prompts
  claiming one article — a *conflict*, not a duplicated copy. `conflicting_binding`
  reads more accurately.

## Decisions (as of spec 065)

- **Keep `prompt` / `Prompt` unchanged for now.** Consistency with the existing
  `prompts/<key>.md` + `article:` frontmatter convention, the `mf-plan` /
  `mf-write` skills, and existing repos outweighs the overload cost for this slice.
- **Keep `thinking` / `Thinking` unchanged.** Same rationale.
- **Keep `duplicate_binding` unchanged** to avoid name churn while the two store
  names are held stable.

## Future rename (deferred, not scheduled)

When a rename is undertaken, the reasoned target is:

- `Prompt` → **`Brief`** (or `Intent`), most likely keeping the `prompts/`
  directory and letting the entity/command name diverge (the `docs/` ↔ `Article`
  pattern), to avoid migrating existing repos and skills.
- `duplicate_binding` → **`conflicting_binding`**, a low-cost tweak to fold in at
  the same time.
- `Thinking` → **`Ledger`** remains an option but is lower priority; `thinking`
  is weak, not wrong.

Any such rename is a breaking output/schema change and must follow the
Constitution's versioning and migration rules.
