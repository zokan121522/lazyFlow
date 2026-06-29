---
name: project-workflow
description: >
  Generic project workflow automation: SDD + GitHub issues/comments/PRs + conventional commits + Obsidian documentation.
  For ANY project. Configure the variables at the start of each project session.
  Trigger: When starting a new phase, feature, or change that needs the full SDD workflow. Or when user says "iniciar fase", "nueva fase", "phase N", "start workflow".
license: Apache-2.0
metadata:
  author: gentleman-programming
  version: "3.0"
---

## ⚠️ CRITICAL — Mandatory Co-loading

**ALWAYS load `senior-architecture` skill BEFORE or ALONGSIDE this skill.**

This is non-negotiable. Every SDD phase (Spec, Design, Tasks, Apply, Verify) must have architecture rules enforced before writing code. The `project-workflow` orchestrates the process; `senior-architecture` enforces the quality.

```javascript
// Must be the first action:
loadSkill("senior-architecture");
// Then proceed with project-workflow
```

If `senior-architecture` is not loaded, STOP and load it before continuing.

---

## ⚠️ CRITICAL — Sub-Phase SDD Independence Rule

**When a phase has sequential sub-phases (e.g. Phase X.0 → Phase X.1), each sub-phase runs its own COMPLETE and INDEPENDENT SDD cycle.** Never merge them.

### Correct vs Incorrect

| ❌ Wrong | ✅ Right |
|----------|----------|
| One Planning Batch covering both 16.0 and 16.1 | **Phase 16.0**: Spec → Design → Tasks → Apply → Verify → Archive |
| One giant Spec with all requirements mixed | **Phase 16.1** (starts AFTER 16.0 Archive): Spec → Design → Tasks → Apply → Verify → Archive |
| One Tasks list with R1-R12 + S1-S8 combined | Phase 16.1 gets its OWN issue comments, OWN branch (or same branch if sequential), OWN PR |

### Why This Matters

1. **Phase X.0 may change the architecture** → Phase X.1's design depends on X.0's actual output, not the planned one
2. **Compaction-proofing**: Each sub-phase is self-contained — if context is lost, you know exactly which sub-phase you're in
3. **Blocking**: Phase X.1 is BLOCKED until Phase X.0 Archive completes — this prevents starting work on unstable foundations
4. **Verification**: Each sub-phase is verified independently — if X.0 breaks something, you catch it before adding X.1 complexity

### Rule Enforcement

```javascript
// If the phase has sub-phases (X.0, X.1, ...):
// 1. Create ONE issue for the overall phase (covers scope)
// 2. Phase 2 Planning Batch covers ONLY Phase X.0
// 3. Phase X.0 runs full SDD: Apply → Verify → Archive
// 4. ONLY AFTER Phase X.0 Archive, create Phase 2 batch for Phase X.1
// 5. Phase X.1 runs its own full SDD cycle

// Exception: If the Obsidian Phase 0 doc documents BOTH sub-phases'
// Spec + Design + Tasks upfront, post them as SEPARATE comments:
//   Comment 1: "## Phase X.0 — Spec\n\n## Phase X.0 — Design\n\n## Phase X.0 — Tasks"
//   Comment 2: "## Phase X.1 — Spec\n\n## Phase X.1 — Design\n\n## Phase X.1 — Tasks"
// But NEVER implement Phase X.1 until Phase X.0 is archived.
```

### Visual Flow

```
Phase Issue #N
├─ (body) Overall scope + test scenarios
│
├─ 📦 BATCH 1 — Phase X.0 ONLY
│  ├─ Comment: "Phase X.0 Spec + Design + Tasks"
│  ├─ Apply X.0 (R1, R2, ...)
│  ├─ Verify X.0
│  └─ Archive X.0 (PR + merge)
│
├─ 📦 BATCH 2 — Phase X.1 ONLY (AFTER X.0 Archive)
│  ├─ Comment: "Phase X.1 Spec + Design + Tasks"
│  ├─ Apply X.1 (S1, S2, ...)
│  ├─ Verify X.1
│  └─ Archive X.1 (PR + merge)
│
└─ Issue closed after Phase X.1 Archive
```

---

## What This Skill Does

Orchestrates the complete development workflow for a project phase or feature:

```
SDD workflow  →  GitHub (issue + comments + PR)  →  Git commits  →  Obsidian docs
```

It is **project-agnostic** — configure the variables below per project.

---

## 📦 Planning Batch Rule (CRITICAL — compaction-proofing)

**After Phase 1 (Init), if Phase 0 documented full Spec + Design + Tasks in Obsidian, post all three planning comments to GitHub in ONE BATCH without asking per step.**

This is the core optimization of this workflow:

| Problem | Solution |
|---------|----------|
| 3 blocking questions (Spec, Design, Tasks) → friction | Batch them → one decision |
| Compaction between phases → lost context | Everything on GitHub at once → source of truth survives compaction |
| "¿Añado comment?" × 3 → user fatigue | Just do it — content is already written in Obsidian |

**Exception**: If any section is missing from the Phase 0 Obsidian doc, ask before posting that specific comment.

---

## Project Variables

Set these at the start of each project session. The skill uses them in all commands and templates.

```yaml
# ─── GitHub ───
GITHUB_REPO: "owner/repo"           # GitHub repository

# ─── Local ───
LOCAL_PATH: "/path/to/project"       # Absolute path to project root

# ─── Obsidian ───
OBSIDIAN_PATH: "~/obsidian/vault/Projects/MyProject"  # Obsidian docs path
DOC_BILINGUAL: false                 # true = EN + ES sections; false = English only (recommended)

# ─── Git ───
BRANCH_PREFIX: "feat/"               # Branch naming prefix
BASE_BRANCH: "main"                  # Base branch for PRs

# ─── SDD ───
SDD_MODE: "engram"                   # engram | openspec | hybrid | none

# ─── Deploy (optional) ───
DEPLOY_ENABLED: false                # true = offer deploy step after Archive
DEPLOY_SSH_HOST: "server-deploy"     # SSH host from ~/.ssh/config (e.g. "homenas", "vps")
```

> **Usage**: When starting work on a project, Álvaro sets these variables at the top of the session and uses them throughout.
>
> **Deploy trigger**: If `DEPLOY_ENABLED: true`, after Phase 5 (Archive) merge is complete, Álvaro will ask: "Señor, el merge está completo. ¿Procedo con el despliegue al servidor?"

---

## ⚡ Pre-flight Check — ALWAYS RUN FIRST (especially after compaction)

This is NOT a phase. It is a **constant cycle step** that Álvaro runs BEFORE any phase, every time work resumes. Its purpose is to re-orient after context loss (compaction, new session, etc.) and verify sync between all sources of truth.

### Checklist (run in order)

| # | Action | Tool | Why |
|---|--------|------|-----|
| 1 | **Read Engram context** | `engram_mem_context(limit=10)` or `engram_mem_search("project-name Phase N")` | Get last session summaries, decisions, bugs |
| 2 | **Read Obsidian docs** | `read("README.md")` + `read("_index.md")` + relevant `Phase N.md` | Get phase status, checkboxes, pending items |
| 3 | **Check GitHub issue** | `gh issue view {N} --json state,comments --jq '{state, commentCount}'` | Verify issue is open, check latest comments |
| 4 | **Check git state** | `git branch --show-current && git log --oneline -3` | Know current branch and last commits |
| 5 | **Verify sync** | Cross-reference Engram + Obsidian + GitHub + git → determine current phase and next step | Detect drift between sources |
| 6 | **🔍 Skills Audit** | `read(".project-skills.md")` o `glob(".opencode/skills/*/SKILL.md")` | Verificar skills del proyecto instaladas |
| 7 | **🧠 Skills Discovery** | Analizar stack + buscar skills globales → recomendar | Solo si NO existe `.project-skills.md` |
| 8 | **Report to user** | "Señor, retomamos {project}. Estamos en {phase}, último avance: {summary}. Skills: {N} activas. ¿Continuamos?" | User confirms before proceeding |

### Skills Audit — Detalle

Este paso verifica que las skills del proyecto están correctamente instaladas y actualizadas.

**Si existe `.project-skills.md`**:
1. Leerlo completo con `read(".project-skills.md")`
2. Verificar que cada skill listada tiene su `SKILL.md` en la ruta indicada
3. Si falta alguna → reported al usuario: "La skill X está referenciada pero no instalada. ¿La instalo?"
4. Durante el desarrollo, antes de cada tarea, verificar si el contexto coincide con alguna skill del proyecto

**Si NO existe `.project-skills.md`** → ejecutar Skills Discovery (paso 7)

### Skills Discovery — Detalle

Este paso SOLO se ejecuta cuando el proyecto no tiene `.project-skills.md`. Analiza el proyecto y recomienda skills.

**Procedimiento**:
1. Analizar el stack: buscar `package.json`, `Cargo.toml`, `requirements.txt`, `go.mod`, `pom.xml`, etc.
2. Listar skills globales: `ls ~/.config/opencode/skills/`
3. Matchear skills relevantes contra el stack detectado (ver tabla de match en AGENTS.md global)
4. Presentar al usuario:
   ```
   "Señor, he analizado el proyecto {nombre}.
    Stack: Python + Flask + JS (~12K líneas).
    
    Recomiendo instalar:
    1. senior-architecture → El proyecto tiene archivos >500 líneas
    2. fusionhub-session → Formato de sesiones para studyflow-hub
    3. sdd-apply → Para implementar las fases SDD
    
    ¿Procedo con la instalación?"
   ```
5. Si el usuario APRUEBA:
   - Crear `.project-skills.md`
   - Copiar skills globales a `.opencode/skills/`
   - Añadir referencia en `AGENTS.md` del proyecto (o crearlo si no existe)
6. Si el usuario RECHAZA: proceder con skills globales por defecto

### Recovery procedure after compaction

When Álvaro detects context was lost (new session, no conversation history):

1. Run all 8 steps above BEFORE writing any code
2. If sources disagree (e.g., Engram says Step 3 done but README says pending), **flag the discrepancy** to the user
3. Do NOT assume any source is correct — present the facts and let the user decide
4. Save a new Engram observation with the reconciliation result: `engram_mem_save(topic_key="recovery/project-name")`

### Troubleshooting sync issues

| Symptom | Likely cause | Action |
|---------|-------------|--------|
| Engram says X, README says Y | One source outdated | Show both, ask user which is correct, update the outdated one |
| GitHub issue has no comments for later phases | Phase was done without documenting | Ask user: "¿Quiere que documente las fases que faltan?" |
| Git branch is `main` but Phase 4 is in progress | Branch was merged, new branch needed | `git checkout -b feat/phase4-name` |
| Engram has no entries for this project | First session | Create initial Engram save |

---

## Workflow Phases (SDD)

Each phase has: **GitHub action** (issue/comment) + **Git action** (commit/branch) + **Obsidian action** (document) + **SDD skill** (if applicable).

### Phase 0 — Consenso / Consensus ⚠️ MUST RUN FIRST

**Purpose**: Before writing a single line of code or creating any issue, discuss and reach a clear consensus with the user on what needs to be done. The Phase doc in Obsidian IS the SDD — it contains Spec, Design, and Tasks directly. No separate documents.

| Aspect | Action |
|--------|--------|
| **Discusión** | Discuss the feature, fix, or change with the user. Explore options, tradeoffs, approaches. Reach a clear verbal consensus. |
| **⚠️ Refactor check** | If the scope involves refactoring, **load `senior-architecture` skill** and run `find {LOCAL_PATH} -type f \( -name "*.js" -o -name "*.py" -o -name "*.css" -o -name "*.html" \) | sort | xargs wc -l` to get the full project tree with ALL files and their line counts. Any file >500 lines or function >30 lines must be noted in the Design section. |
| **Obsidian** | Create `Phase {N} - {Name}.md` in Obsidian project folder using the SDD template below. The doc structure is: **Spec → Design → Tasks → Out of Scope**. |
| **GitHub** | (none — no issue yet) |
| **Git** | (none — no code yet) |
| **SDD skill** | (none — template is written directly) |

**Template document** (single phase — no sub-phases):

```markdown
# Phase {N} — {Title}

> {summary of what this phase accomplishes}

---

## Workflow Phases

| # | Phase | Purpose | Type | Status |
|--|-------|---------|------|--------|
| 0 | **Consenso** | Discuss requirements + agree on approach, write doc | Preparación | ✅ |
| 1 | **Init** | Create GitHub issue + branch | Scaffolding | ✅ |
| 2 | **📦 Planning Batch** | Post Spec + Design + Tasks to GitHub | Publicación | ✅ |
| 2.5 | **🧠 Concept Learning** | Detect new concepts in Tasks, document in Obsidian + MP3 before Apply | Aprendizaje | 📝 |
| 3 | **Apply** | Implement code changes | ✅ **SDD Apply** | 📝 |
| 4 | **Verify** | Validate implementation against test scenarios | ✅ **SDD Verify** | 📝 |
| 5 | **Archive** | Create PR, merge, close issue | ✅ **SDD Archive** | 📝 |
| 6 | **🚀 Deploy** | *(if enabled)* SSH + git pull + restart | Despliegue | 📝 |
| 7 | **Retrospective** | Audit process, generate delta tree, update doc | Post-mortem | 📝 |

---

## Phase 0 — Consenso | Preparación

**Discussed**: {what was discussed with the user — options, tradeoffs, constraints}
**Agreed**: {the final decision — what will be built, approach, scope}

---

## Phase 1 — Init | Scaffolding

- **Issue**: #{number} — {title} (link)
- **Branch**: {branch name} (from {base})
- **Base**: {base}

---

## Phase 2 — 📦 Planning Batch | Publicación

{All three planning artifacts posted as GitHub issue comments in a single batch on YYYY-MM-DD}

| Artifact | Link | Type |
|----------|------|------|
| **Spec + Design + Tasks** | {comment link} | {single or per sub-phase} |

---

## Spec

### Description
{what this phase does — feature, refactor, fix}

### Requirements
- {requirement 1}
- {requirement 2}

### Test Scenarios
- [ ] Scenario 1: {description}
- [ ] Scenario 2: {description}

---

## Design

### Current Architecture — Full Project Tree

Show ALL project files with their current line counts, organized by directory. This captures the state BEFORE any changes.

**Command to generate** (run at project root):
```bash
# Generates full project tree with right-aligned line counts
python3 << 'PYEOF'
import os
project = "."
extensions = ('.js', '.py', '.css', '.html')
files = []
for root, dirs, filenames in os.walk(project):
    if 'node_modules' in root or '.git' in root or '__pycache__' in root:
        continue
    for f in filenames:
        if f.endswith(extensions):
            path = os.path.join(os.path.relpath(root, project), f)
            if path.startswith('.'): continue
            try:
                with open(os.path.join(root, f)) as fh:
                    count = sum(1 for _ in fh)
                if count > 0: files.append((path, count))
            except: pass
files.sort()
# ... (build tree and render with └──/├──, right-aligned counts)
for path, count in files: print(f"{path}: {count}")
PYEOF
```

**Example** (from a real project):
```
├── backend/
│   ├── routes/
│   │   ├── agenda.py                   416
│   │   ├── ai.py                     1,647
│   │   ├── ...
│   ├── models.py                       572
│   └── server.py                       118
├── frontend/
│   ├── features/
│   │   ├── agenda/
│   │   │   ├── agenda.css              176
│   │   │   └── agenda.js             1,112
│   │   └── ...
│   ├── app.js                          321
│   ├── index.html                      362
│   └── styles.css                      101
└── scripts/
    └── migrate_from_java.py            346
```

Note: This is the COMPLETE project tree. Every file in the project is listed. The total line count at the bottom gives the full project size at this point in time. This tree serves as the baseline for calculating the delta at the end of the phase.

### Target Architecture

{what changes — new files, modified files, deleted files. Show tree with estimated line counts.}

```
{tree of files after changes with estimated line counts}
```

### Migration Plan

{step-by-step approach if refactoring is involved}

---

## Tasks

- [ ] Task 1: {description}
- [ ] Task 2: {description}

---

## Out of Scope

- ❌ {feature 1 deferred}
- ❌ {feature 2 deferred}
```

**Alternative — Sub-Phase Structure** (use when a phase has sequential sub-phases, e.g. Phase 15.0 + Phase 15.1):

```markdown
# Phase {N} — {Title}

> {summary of what this phase accomplishes}

---

## Workflow Phases

| # | Phase | Purpose | Type | Status |
|---|-------|---------|------|--------|
| 0 | **Consenso** | Discuss requirements + agree on approach, write doc | Preparación | ✅ |
| 1 | **Init** | Create GitHub issue + branch | Scaffolding | ✅ |
| 2 | **📦 Planning Batch** | Post Spec + Design + Tasks to GitHub | Publicación | ✅ |
| 3 | **Apply — {N}.0** | {first sub-phase summary} | ✅ **SDD Apply** | 📝 |
| 3 | **Apply — {N}.1** | {second sub-phase summary} | ✅ **SDD Apply** | 📝 |
| 4 | **Verify** | Validate implementation against test scenarios | ✅ **SDD Verify** | 📝 |
| 5 | **Archive** | Create PR, merge, close issue | ✅ **SDD Archive** | 📝 |
| 6 | **🚀 Deploy** | *(if enabled)* SSH + git pull + restart | Despliegue | 📝 |
| 7 | **Retrospective** | Audit process, generate delta tree, update doc | Post-mortem | 📝 |

---

## Phase 0 — Consenso | Preparación

**Discussed**: {what was discussed with the user — options, tradeoffs, constraints}
**Agreed**: {the final decision — refactor first then features, SDD cycle per sub-phase, etc.}

---

## Phase 1 — Init | Scaffolding

- **Issue**: #{number} — {title} (link)
- **Branch**: {branch name} (from {base})
- **Base**: {base}

---

## Phase 2 — 📦 Planning Batch | Publicación

All three planning artifacts posted as GitHub issue comments in a single batch on YYYY-MM-DD:

| Artifact | Link | Type |
|----------|------|------|
| **Phase {N}.0 Spec + Design + Tasks** | {comment link} | Refactor |
| **Phase {N}.1 Spec + Design + Tasks** | {comment link} | Features |

---

## Phase {N}.0 — {Sub-phase Name} | ✅ SDD Apply

### Phase 0.{N} — Consenso

**Discussed**: {what was discussed specific to this sub-phase — options, constraints, decisions}
**Agreed**: {the final decision for this sub-phase}

### Phase 1.{N} — Init

Same as top-level Phase 1 (shared for all sub-phases):
- **Issue**: #{number}
- **Branch**: {branch name}
- **Base**: {base}

### Phase 2.{N} — 📦 Planning Batch

| Artifact | Link | Date |
|----------|------|------|
| Spec + Design + Tasks | {comment link} | YYYY-MM-DD |

### Spec

#### Description
{what this sub-phase does}

#### Requirements
- {requirement 1}

#### Test Scenarios
- [ ] {scenario 1}

### Design

#### Current Architecture
{full project tree with line counts — same as main template}

#### Target Architecture (after {N}.0)
{tree of files after this sub-phase's changes — NO future sub-phase files yet}

#### Migration Plan
{step-by-step approach}

### Tasks

- [ ] Task 1

---

## Phase {N}.1 — {Sub-phase Name} | ✅ SDD Apply

> ⚠️ Requires Phase {N}.0 to be complete. This phase assumes the target architecture from Phase {N}.0.

### Phase 0.{N+1} — Consenso

**Discussed**: {what was discussed specific to this sub-phase}
**Agreed**: {the final decision}

### Phase 1.{N+1} — Init

Same as top-level Phase 1 (shared for all sub-phases).

### Phase 2.{N+1} — 📦 Planning Batch

| Artifact | Link | Date |
|----------|------|------|
| Spec + Design + Tasks | {comment link} | YYYY-MM-DD |

### Spec

#### Description
{what this sub-phase does}

#### Requirements
- {requirement 1}
- {requirement 2}

#### Test Scenarios
- [ ] {scenario 1}

### Design

#### Current Architecture (after Phase {N}.0)
{references target of {N}.0 — tree with only those files}

#### Target Architecture (after {N}.1)
{tree with new files added by this sub-phase}

#### Implementation Order
{step-by-step with dependencies}

### Tasks

- [ ] Task 1

---

## Out of Scope

- ❌ {feature deferred}
```

**When to use which**:
- **Single phase** (no sequential sub-phases): Use the main template above. Phase 0/1/2 are at the top level (shared), then Spec/Design/Tasks follow.
- **Sequential sub-phases** (e.g. 15.0 refactor → 15.1 features): Use the sub-phase template. Phase 0/1/2 at the top cover the overall phase; **each sub-phase also includes its own Phase 0.X/1.X/2.X sections** so the sub-phase is self-contained and the full workflow is visible.
- **Workflow Phases table**: Always include at the top. Phases 0–2 are marked ✅ after completion. Apply rows show which sub-phase. Verify/Archive/Retrospective start as 📝 and update as work progresses.
- The current architecture tree goes in Phase {N}.0 (it captures the initial state). Each subsequent sub-phase references the previous sub-phase's target as its current architecture.
- The `| ✅ **SDD Apply** |` type column makes it immediately clear which phases are pure SDD vs contextual.

---

### Phase 1 — Init

| Aspect | Action |
|--------|--------|
| **GitHub** | Create issue with title + summary + test scenarios |
| **Git** | `git checkout {BASE_BRANCH} && git pull && git checkout -b {BRANCH_PREFIX}{name}` |
| **Obsidian** | (already created in Phase 0 — add issue reference to the doc) |
| **SDD skill** | (done manually — create issue) |

---

### Phase 2 — Planning Batch 📦 (Compaction-Proofing)

**Purpose**: Publish all planning artifacts (Spec + Design + Tasks) from Obsidian to GitHub in ONE batch.
The content already exists in the Phase 0 Obsidian doc — no new decisions are needed.

**Rule**: If Phase 0 documented full Spec + Design + Tasks → post all 3 comments without asking.
If any section is missing → ask before posting that comment.

| Aspect | Action |
|--------|--------|
| **GitHub** | Post 3 comments in sequence: `## Spec`, `## Design`, `## Tasks` |
| **Git** | (no commit) |
| **Obsidian** | Add issue reference (#N) to the Phase doc |
| **SDD skill** | (none — comments only) |

**Template batch** (single phase — no sub-phases):
````markdown
## Spec

### Description
{what this phase/feature does — extracted from Obsidian doc}

### Requirements
- {requirement 1}
- {requirement 2}

### Test Scenarios
- [ ] Scenario 1: {description}
- [ ] Scenario 2: {description}
---

## Design

### Approach
{technical approach — extracted from Obsidian doc}

### Current Architecture
{file tree with line counts}

### Target Architecture
{tree after changes with line counts}

### Files Affected
| File | Change |
|------|--------|
| {path} | {what changes} |

---

## Tasks

- [ ] Task 1: {description}
- [ ] Task 2: {description}
- [ ] Task 3: {description}
````

**Alternative — Sub-Phase Batch** ⚠️ See [Sub-Phase SDD Independence Rule](#⚠️-critical-—-sub-phase-sdd-independence-rule) above.

Post **2 separate comments**, one per sub-phase. NEVER combine them into one.

**Comment 1 — Phase {N}.0** (posted immediately after Init):
````markdown
## Phase {N}.0 — {Name} — Spec

### Description
{extracted from Obsidian}

### Requirements
- {req 1}

### Test Scenarios
- [ ] {scenario 1}

---

## Phase {N}.0 — Design

### Current Architecture
{full project tree}

### Target Architecture (after {N}.0)
{tree after refactor}

### Migration Plan
| Step | Action | Type |
|------|--------|------|
| R1 | {action} | NEW |

---

## Phase {N}.0 — Tasks

- [ ] R1: {description}
- [ ] R2: {description}
````

---

**Comment 2 — Phase {N}.1** ⏳ **Post ONLY after Phase {N}.0 Archive is complete**:

````markdown
## Phase {N}.1 — {Name} — Spec

### Description
{extracted from Obsidian}

### Requirements
- {req 1}

### Test Scenarios
- [ ] {scenario 1}

---

## Phase {N}.1 — Design

### Current Architecture (after Phase {N}.0)
{references target of {N}.0}

### Target Architecture (after {N}.1)
{final tree with all new files}

### Implementation Order
| Step | Feature | Depends on |
|------|---------|------------|
| 1 | F1 | Phase {N}.0 |

---

## Phase {N}.1 — Tasks

- [ ] F1: {description}
- [ ] F2: {description}
````

---

### Phase 2.5 — Concept Learning 🧠 (Learn before Apply)

**Purpose**: Before writing code, detect if any Task requires a concept the user hasn't learned yet. If so, document it in Obsidian with an MP3 explanation before implementing.

| Aspect | Action |
|--------|--------|
| **Detection** | Review each Task in the Planning Batch. If it involves a library, language feature, pattern, algorithm, or logic that may be new → flag it |
| **Check Obsidian** | Look in `🎯 Conceptos/` folder for existing notes on that concept |
| **Ask user** | "Señor, el task X requiere [concepto]. ¿Ya lo conoce o lo documentamos?" |
| **Document** | If new → create structured note using `concept-learner` skill (template: definición + sintaxis + ejemplo + audio MP3) |
| **GitHub** | (no comment — this is a learning step, not a deliverable) |
| **Obsidian** | New note in `🎯 Conceptos/{concepto}.md` with embedded MP3 |
| **SDD skill** | `concept-learner` |

**Regla**: Si el usuario ya conoce el concepto, se salta la fase y se va directamente a Apply. Si no, se genera la nota y el audio, y luego se procede.

**Template de nota** (generada por `concept-learner`):
```markdown
# 🎯 Concepto: [nombre]

> [definición breve]

## 📖 Definición
## 🔧 Sintaxis / Uso
## 💡 Ejemplo real
## 🔊 Audio
```

---

### Phase 3 — Apply (per sub-step)

Each sub-step (e.g., "Step 1: Schema", "Step 2: Routes") follows this mini-cycle:

| Aspect | Action |
|--------|--------|
| **Code** | Implement changes |
| **Git** | `git add ... && git commit -m "{type}: {short description}"` |
| **GitHub** | `gh issue comment {N} --repo {GITHUB_REPO} --body "## Step {N} done\n\n{what was implemented}"` |
| **Obsidian** | Update checklist in `Phase {N} - {Name}.md` and/or project `README.md` — mark checkbox |
| **SDD skill** | `sdd-apply` |

**Commit message format**:
```
{type}: {short description}

- Bullet 1 of what changed
- Bullet 2
```

Types: `feat:` (new feature), `fix:` (bug fix), `chore:` (infra/tooling), `refactor:` (code change with no behavior change), `docs:` (documentation only).

**⚠️ CRITICAL — Commit per sub-phase step (R1, R2, S1, S2, ...)**:

Each sub-phase (e.g., Phase X.0, Phase X.1) is broken down into numbered steps (R1, R2, ..., S1, S2, ...). **Every step MUST be committed individually** with a brief English message describing what was done. This ensures:
- Each step is independently revertible
- The commit history clearly shows progression through the phase
- If context is lost mid-phase, the git log shows exactly where you left off

Example commit sequence within Phase X.0:
```
refactor: extract backend/ai/ollama.py with stream tracking + HTTP calls

- Move _register_stream, _call_ollama, _call_ollama_stream to ollama.py
- Extract progress helpers (_build_progress, _set_progress, etc.)
- Update routes/ai.py to import from backend.ai.ollama

refactor: extract backend/ai/utils.py with PDF extraction + prompts

- Move _resolve_pdf_path, _extract_pdf_text, _filter_pdf_repeated_lines
- Extract prompt builders (_build_content_prompt, _build_test_prompt, etc.)
- Move token estimation (_estimate_tokens) and coverage analysis
```

**Phase completion TAG**:
Once the **entire Phase** (all sub-phases) is complete and the final Archive is done:
1. Create one final commit updating any remaining docs/checklists
2. Create a git tag: `git tag phase{N}` (e.g., `phase16`)
3. Push the tag: `git push origin phase{N}`
4. This tag marks the boundary between phases — makes it easy to diff or revert

---

### Phase 4 — Verify

| Aspect | Action |
|--------|--------|
| **Test** | Run tests, curl endpoints, manual browser check |
| **GitHub** | `gh issue comment {N} --repo {GITHUB_REPO} --body "## Verify\n\n{test results}"` |
| **Obsidian** | Mark phase as verified |
| **SDD skill** | `sdd-verify` |

---

### Phase 5 — Archive

| Aspect | Action |
|--------|--------|
| **GitHub** | `gh pr create --repo {GITHUB_REPO} --title "{title}" --body "{body}" --head {BRANCH} --base {BASE_BRANCH}` |
| **GitHub** | `gh pr merge {PR} --squash` (or ask user to merge via UI) |
| **Obsidian** | Mark phase as ✅ completed in `README.md` and `_index.md` |
| **SDD skill** | `sdd-archive` |

---

### Phase 6 — Deploy 🚀 (optional — requires DEPLOY_ENABLED=true)

**Purpose**: After the PR is merged and the code is in `main` on GitHub, deploy the latest version to the server. This phase only exists when the project has `DEPLOY_ENABLED: true` configured.

| Aspect | Action |
|--------|--------|
| **SSH** | `ssh {DEPLOY_SSH_HOST}` — connects to server and runs the deploy script/commands |
| **GitHub** | (no comment — deploy is operational, not a deliverable) |
| **Git** | (no commit — code is already merged to main) |
| **Obsidian** | Mark phase as ✅ in `README.md` / `_index.md` |

**Deploy procedure** (agnostic — adapt to your stack):

```bash
# Generic: SSH + git pull + restart
ssh {DEPLOY_SSH_HOST}
# Inside the server:
#   cd /path/to/project
#   git pull origin main
#   # restart your service (docker, systemctl, supervisor, etc.)
#   docker compose up -d --build   # if using Docker
#   systemctl restart myapp        # if using systemd
#   supervisorctl restart myapp    # if using supervisor
#   pnpm build && pm2 restart      # if using Node/PM2
```

**Convenience shortcut** (if configured in `~/.ssh/config`):

If your SSH config has a `RemoteCommand` that handles pull + restart automatically:
```bash
ssh {DEPLOY_SSH_HOST}
```

**Trigger**: Álvaro will ask *after* Archive merge is complete:
> "Señor, el merge está completo. DEPLOY_ENABLED está activo. ¿Procedo con el despliegue al servidor?"

If the user says yes → run the SSH command. If no → mark as ⏭️ Skipped and proceed to Retrospective.

---

### Phase 7 — Retrospective / Revisión ⚠️ MUST RUN LAST — ALWAYS

This phase **always** runs after Archive. It does NOT produce code. It has two parts: (1) audit the process for missed steps, and (2) review the initial Phase 0 documentation to capture changes, late fixes, issues discovered, and any pending checkboxes.

| Aspect | Action |
|--------|--------|
| **Audit** | Read the todo list AND compare against ALL phases of this skill |
| **GitHub** | Check issue comments exist for each phase |
| **Obsidian** | **Re-read** the Phase 0 doc. Update it with late fixes, issues found mid-implementation, changes to the original plan. Add a "Retrospective" section. |
| **Report** | Present checklist of what's done vs what's missing, then ask user |

#### Step-by-step procedure

1. **Read the current todo list** — get all tasks and their statuses
2. **Compare against this skill's workflow phases** — check each phase:

   | Phase | What to check |
   |-------|---------------|
   | **0 — Consenso** | ✅ Discussed with user? Obsidian doc created (Phase {N}.md) with Phase 0 section documenting what was discussed + agreed? |
   | **1 — Init** | ✅ GitHub issue created? Branch created? Phase 1 section in doc with issue # + branch? |
   | **2 — Planning Batch** | ✅ Spec + Design + Tasks comments posted in batch? Phase 2 section in doc with comment links? |
   | **3 — Apply** | ✅ Code changes committed? Progress comments per sub-step? Obsidian checklist updated? |
   | **4 — Verify** | ✅ Tests run? Verify comment posted? |
   | **5 — Archive** | ✅ PR created? PR merged? `README.md` / `_index.md` updated? Branch cleaned up? |
   | **6 — Retrospective** | ✅ This audit running? Phase 0 doc re-read and updated? Delta tree generated? |

3. **Re-read the Phase 0 documentation** (the initial consensus doc in Obsidian). Compare it against what was actually implemented:
   - Were there any **late fixes** or bugs found during implementation?
   - Did the **scope change** from the original plan?
   - Are there **new learnings** to document?
   - Are there **checkboxes** that remained unchecked? Why?
   - Add a `## Retrospective` section to the doc with findings
   - **Generate the final delta tree**: Run the SAME command used at the start of the phase (`find {LOCAL_PATH} -type f \( -name "*.js" -o -name "*.py" -o -name "*.css" -o -name "*.html" \) | sort | xargs wc -l`) to get the final state of ALL files. Then compare each file against its initial line count from the Current Architecture section in the Phase doc. Every file must show its delta: `+N` (lines added), `-N` (lines removed), or `0` (unchanged). New files (not present in the initial tree) get `+N 🆕`.

4. **For any missing step**, mark it as ❌ in the report and suggest the fix
5. **Present the report** to the user with clear next actions
6. **Ask**: "¿Quiere que corrija algo antes de dar por terminado, Señor?"

#### Audit report template

```markdown
## 📋 Retrospective — {Phase Name}

### Process Audit

| Fase / Phase | Estado |
|-------------|--------|
| Consenso — Discussion + Obsidian doc | ✅ / ❌ |
| Init — Issue + Branch | ✅ / ❌ |
| Planning Batch — Spec + Design + Tasks comments | ✅ / ❌ |
| Apply — Commits + Comments + Obsidian | ✅ / ❌ |
| Verify — Tests + Comment | ✅ / ❌ |
| Archive — PR + Merge + Obsidian | ✅ / ❌ |

### Documentation Review

- **Initial plan vs reality**: {any deviations?}
- **Late fixes / issues found**: {what changed mid-implementation?}
- **Pending items**: {unchecked checkboxes, unfinished work}
- **Learnings**: {what should be done differently next time?}

### Delta Tree — Full Project (lines added/removed vs initial state)

Every file in the project is listed in tree format. Compare final vs initial line counts. Format: `(final, +N/-N)`.

**Example** (after a refactor phase that split `agenda.js` into modules):
```
├── frontend/
│   ├── features/
│   │   ├── agenda/
│   │   │   ├── agenda.css              176  (  0)
│   │   │   ├── agenda.js               120  (-992)
│   │   │   ├── agenda-core.js          280  (+280) 🆕
│   │   │   ├── agenda-calendar.js      250  (+250) 🆕
│   │   │   ├── agenda-session.js       180  (+180) 🆕
│   │   │   ├── agenda-timer.js          50  (+50)  🆕
│   │   │   ├── agenda-habits.js        200  (+200) 🆕
│   │   │   ├── agenda-quicknote.js      50  (+50)  🆕
│   │   │   ├── agenda-dnd.js           120  (+120) 🆕
│   │   │   ├── agenda-timeline.js      200  (+200) 🆕
│   │   │   └── agenda-columns.js       100  (+100) 🆕
│   │   └── ...
│   ├── index.html                      362  (  0)
│   └── app.js                          321  (  0)
├── backend/
│   ├── routes/
│   │   ├── agenda.py                   416  (  0)
│   │   ├── sessions.py                 112  (  0)
│   │   └── ...
│   └── ...
└── ...
────────────────────────────────────────────
Total:                               +1.340 / -992
```

### ❌ Missing items
- {item 1} — fix: {how to fix}

### ✅ Completed items
- {item 1}
- {item 2}
```

#### Critical rule

**This phase is MANDATORY.** Never skip it. Never mark work as done without running the retrospective first. If the user says "done" or "next phase" without it, Álvaro must insist: "Señor, permítame hacer la retrospectiva primero para asegurarnos de que la documentación inicial refleja todo lo que ocurrió."

---

## Visual Workflow

```
                    ┌─────────────────────────────┐
                    │   ⚡ PRE-FLIGHT CHECK        │ ← ALWAYS start here
                    │   Engram → Obsidian → GitHub │
                    │   → Git → Sync → Report      │
                    └──────────┬──────────────────┘
                               │ (sync OK)
                               ▼
        Init → 📦 Planning Batch → 🧠 Concept Learning → Apply → Verify → Archive → 🚀 Deploy → REVIEW 🔍
        │           │                    │              │  │       │        │       │         │       │
        │           │                    │              │  │       │        │       │         │       └─ Todo vs Skill audit
        │           │                    │              │  │       │        │       │         │
        │           │                    │              │  │       │        │       │         └─ SSH + git pull + restart (if enabled)
        │           │                    │              │  │       │        │       └─ PR + merge + Obsidian ✅
        │           │                    │              │  │       │        └─ (Archived)
        │           │                    │              │  │       └─ Test + comment results
        │           │                    │              │  └─ Per sub-step: code → commit → comment → Obsidian
        │           │                    │              └─ Implement code changes
        │           │                    └─ Check concepts → Obsidian note + MP3 → Apply
        │           └─ Post Spec + Design + Tasks (batch, no ask)
        └─ Create issue + branch

🔄 CONSTANT CYCLE: After REVIEW → back to ⚡ PRE-FLIGHT CHECK for next phase

> **Note**: 🚀 Deploy only runs if `DEPLOY_ENABLED: true`. If disabled, Archive → REVIEW directly.
```

---

## Obsidian Documentation Pattern

### Default (DOC_BILINGUAL = false)

```
# Phase N — Title

> Summary

---

## Spec

{requirements, test scenarios}

## Design

{current architecture tree, target architecture tree, migration plan}

## Tasks

- [ ] Task 1
- [ ] Task 2

## Out of Scope

- ❌ Deferred item
```

### Bilingual (DOC_BILINGUAL = true, optional)

```
# Phase N — EN Title / Fase N — ES Título

> EN summary  
> ES resumen

---

## English

{content in English}

---

## Español

{content in Spanish}
```

### README.md & _index.md

- `README.md` → Single source of truth with full checklist per phase (English only by default).
- `_index.md` → Executive summary with phase status table and link to README.

---

## GitHub Issue Convention

One issue per phase/feature. Comments for each SDD phase:

```
Issue #N: Phase X — Title
├─ (body) Summary + test scenarios       ← Init
├─ 📦 PLANNING BATCH (posted at once)
│  ├─ Comment: "## Spec..."              
│  ├─ Comment: "## Design..."            
│  └─ Comment: "## Tasks..."             
├─ Comment: "## Step 1 done..."          ← Apply (per sub-step)
├─ Comment: "## Step 2 done..."          ← Apply (per sub-step)
├─ Comment: "## Verify..."               ← Verify
├─ PR #N merged                          ← Archive
└─ Comment: "## Review / Audit..."       ← Retrospective 🔍 (MUST RUN LAST)
```

---

## Commands (parameterized)

```bash
# ─── Branch ───
git checkout {BASE_BRANCH} && git pull && git checkout -b {BRANCH_PREFIX}{name}

# ─── Commit ───
git add -A && git commit -m "type: description"

# ─── Issue Comment ───
gh issue comment {N} --repo {GITHUB_REPO} --body "## {section}\n\n{content}"

# ─── Pull Request ───
gh pr create --repo {GITHUB_REPO} --title "Phase {N}: {title}" --body "{body}" --head {BRANCH_PREFIX}{name} --base {BASE_BRANCH}

gh pr merge {PR_N} --squash --subject "feat: {title}" --body "{body}"

# ─── Obsidian ───
# Create phase doc:
touch "{OBSIDIAN_PATH}/Phase {N} - {Name}.md"

# Update checkout in README:
# {edit README.md to mark checkbox}

# ─── Deploy (only if DEPLOY_ENABLED: true) ───
ssh {DEPLOY_SSH_HOST}                              # git pull + restart via RemoteCommand

# Or step-by-step:
#   ssh user@host
#   cd /path/to/project
#   git pull origin main
#   # restart service (docker, systemctl, supervisor, pm2, etc.)
```

---

## ⚡ Flow Kanban Integration (portable — works for ANY project)

### Overview

The `flow` TUI/CLI (`juanknebel/flow`) is the physical kanban board. Cards are stored as Markdown files with YAML frontmatter on disk (`~/.flow/boards/principal/`). Álvaro interacts with cards via `flow-cli` (JSON output) and direct filesystem reads.

### Column → SDD Phase Mapping

| Flow Column | Card → What Álvaro does | SDD Phase |
|-------------|------------------------|-----------|
| **💡 Idea** | User drops raw idea. Álvaro reads it here. | — (raw input) |
| **TODO · Spec/Design/Tasks** | Álvaro creates GitHub Issue + posts Spec + Design + Tasks comments | Init + Planning Batch |
| **IN PROGRESS · Apply** | Álvaro implements code — commits per sub-step | Apply |
| **IN REVIEW · Verify** | User reviews. Álvaro has already tested. | Verify |
| **DONE · Archived** | PR merged, issue closed | Archive |

### Card Transitions — `flow-cli move` + `flow-cli edit` (Álvaro ejecuta en cada paso)

Cada transición actualiza la descripción de la card con `flow-cli edit --body` para que la card muestre el historial completo de por dónde vamos.

| # | Transition | Who | Álvaro runs + edits body | Column |
|---|-----------|-----|--------------------------|--------|
| 1 | **Idea** | **User** drops card with title + description (+ optional `project`) | *(waits for user)* — body = idea inicial | 💡 Idea |
| 2 | **Notify** | **User** says "Álvaro, mira la card [title]" | `flow-cli show <id> -f json` (read body) | 💡 Idea |
| 3 | **Consenso** | **Both** discuss: feature/fix/refactor? Scope? | *(discussion, no move)* | 💡 Idea |
| 4 | **→ TODO** | **Álvaro** creates GitHub Issue + branch | `flow-cli move <id> todo` + edit body: append `📦 Issue: #N\n📎 Rama: feat/phaseN-name` | **TODO** |
| 5 | **📦 Planning** | **Álvaro** posts Spec + Design + Tasks | edit body: append `📋 Spec + Design + Tasks: https://github.com/{owner}/{repo}/issues/{N}#issuecomment-{id}` | TODO |
| 6 | **→ IN PROGRESS** | **Álvaro** implements code (commit per sub-step) | `flow-cli move <id> in_progress` + edit body: append `🔧 En progreso: {current step}` | **IN PROGRESS** |
| 7 | **🚀 Deploy Test** | **Álvaro** tests locally with compose | edit body: append `🧪 Deploy test: ✅ / ❌` | IN PROGRESS |
| 8 | **→ IN REVIEW** | **Álvaro** runs tests, posts Verify comment | `flow-cli move <id> in_review` + edit body: append `👀 En revisión` | **IN REVIEW** |
| 9 | **Review** | **User** checks. Says "OK" or changes. | *(waits for user)* | IN REVIEW |
| 10 | **→ DONE** | **Álvaro** creates PR → merge → close issue | `flow-cli move <id> done` + edit body: append `✅ Completado\n🔄 PR: #{pr_number}` | **DONE** |

**Verify Failure transitions** (when user rejects or Deploy Test fails):

| Failure | Álvaro runs + edits body | Goes back to |
|---------|-------------------------|-------------|
| **Code bug** | `flow-cli move <id> in_progress` + edit body: append `🔁 Bug detectado, arreglando...` | IN PROGRESS (Apply) |
| **Design flaw** | `flow-cli move <id> todo` + edit body: append `🔁 Rediseñando...` | TODO (Design) |
| **Requirement wrong** | `flow-cli move <id> todo` + edit body: append `🔁 Re-especificando...` | TODO (Spec) |
| **Deploy test fails** | `flow-cli move <id> in_progress` + edit body: append `🔁 Deploy test falló, arreglando...` | IN PROGRESS (fix + test again) |

**Ejemplo de card al llegar a DONE**: La descripción se vería así tras acumular los `--body` de cada paso:

```
Implementar dark mode toggle

📦 Issue: #42
📎 Rama: feat/phase12-dark-mode
📋 Spec + Design + Tasks: https://github.com/owner/repo/issues/42#issuecomment-12345
🔧 En progreso: Step 1 — crear toggle switch
🔧 En progreso: Step 2 — añadir estilos CSS
🧪 Deploy test: ✅
👀 En revisión
✅ Completado
🔄 PR: #43
```

### Verify Failure — What to Go Back To

When Verify fails (or Deploy Test fails), the card **stays in its current column**. Álvaro moves it BACK to the appropriate column depending on the failure type:

| Failure Type | Examples | Go Back To | What Álvaro Does |
|-------------|----------|-----------|------------------|
| **Bug/code error** | Wrong logic, syntax error, broken API call | **Apply** (IN PROGRESS) | Fix code, test again, move back to IN REVIEW |
| **Design flaw** | Architecture doesn't fit, wrong library, approach doesn't scale | **Design** (TODO) | Redesign, redo Tasks, redo Apply, redo Verify |
| **Requirement wrong** | Missing feature, misunderstood spec, wrong acceptance criteria | **Spec** (TODO) | Re-spec, re-Design, re-Tasks, re-Apply, re-Verify |
| **Deploy test fails** | Docker compose breaks, port conflicts, env vars missing | **Apply** (IN PROGRESS) | Fix deploy config, test again, move to IN REVIEW |

**Key rule**: never go back more steps than necessary. If it's just a code bug, don't touch the Spec — go straight to Apply.

**Visual**:
```
Spec ─→ Design ─→ Tasks ─→ Apply ─→ Verify ─→ Deploy Test ─→ Archive
  ↑                      ↑        │              │
  └── Req wrong ─────────┘        │              │
                                  └── Code bug ──┘
                                             │
                                   Deploy fails ──┘
```

### 🚀 Deploy Test Protocol (NEW — mandatory before Archive)

**Purpose**: Before merging code to `main`, verify it works locally in the SAME Docker environment used in production. Ensures we NEVER merge broken code.

**Flow**:
```
Apply → 👇 Deploy Test ──✅ OK──→ Verify → Archive → Deploy to server
                 │
                 └──❌ Fail → fix in branch → test again → merge
```

**Procedure** (Álvaro runs this from the feature branch):

```bash
# 1. Build and start with production compose
docker compose -f docker-compose.prod.yml up --build -d

# 2. Health check
curl -f http://localhost:{PORT}/health  # or the app's main URL

# 3. Run a quick smoke test (relevant to the phase)
#    e.g., check the feature works, data loads, page renders
curl -s http://localhost:8080/ | grep "expected_text"

# 4. If OK → proceed to Verify + Archive
#    If fail → stop compose, fix in branch, test again

# 5. Clean up
docker compose -f docker-compose.prod.yml down
```

When `DEPLOY_ENABLED: true`, the Archive phase in the Workflow Phases table becomes:
```
| 5 | **Archive** | PR + merge + close issue | ... |
| 5.5 | **🚀 Deploy Test** | Test locally with production compose before merge | Pruebas | 📝 |
| 6 | **🚀 Deploy** | ssh server-deploy → git pull → restart | Despliegue | 📝 |
```

**Critical rule**: If Deploy Test fails, DO NOT proceed to Archive. Fix the issue first, test again, then merge.

### Tool: `flow-cli` Reference

```bash
# Show card details (JSON)
flow-cli show <PROJECT-ID> -f json

# List all cards by column
flow-cli list -f json

# Create a card (used when Álvaro needs to create tracking cards)
flow-cli create <col_id> "Title" --body "Description" --project "project-name"

# Move a card between columns
flow-cli move <PROJECT-ID> <col_id>

# Edit a card
flow-cli edit <PROJECT-ID> --title "New title" --body "New description"
```

### Tips

- The `project` field in flow cards should match the project name in `GITHUB_REPO` for consistency
- Álvaro prefers `flow-cli show <id> -f json` for reading cards (structured JSON)
- Direct filesystem reads to `~/.flow/boards/principal/cols/<col_id>/<card_id>.md` are faster and work when the TUI is not running
- When a card is in IN REVIEW, the user only needs to do ONE thing: approve or reject. No extra work.
```
