---
name: senior-architecture
description: >
  Enforces universal SENIOR-level architecture rules across ALL projects and languages.
  Trigger: SDD phases (Spec, Design, Tasks, Apply, Verify), refactoring tasks, or when any file exceeds 500 lines / any function exceeds 30 lines.
---

## When to Use

- **ALWAYS** when starting any SDD phase (Spec → Design → Tasks → Apply → Verify)
- When creating new files or modifying existing ones
- When ANY file in the project exceeds **500 lines**
- When ANY function/method exceeds **30 lines**
- When adding a new feature to a codebase you haven't refactored yet

---

## Universal Architecture Rules (SENIOR Level)

These rules apply to **ANY language, ANY framework, ANY project**. No exceptions.

### Rule 1 — Hard Limit: 500 lines per file

```
ANY file exceeding 500 lines MUST be refactored BEFORE adding new code.
```

This is the FIRST check before any modification. Count lines with `wc -l` or the equivalent for the language.

**Language-agnostic check:**
```bash
# Count lines in all source files
find . -name "*.js" -o -name "*.py" -o -name "*.go" -o -name "*.ts" -o -name "*.css" -o -name "*.html" | xargs wc -l | sort -rn
```

### Rule 2 — Extraction Threshold: 30 lines per function

```
Any function, method, or block exceeding ~30 lines MUST be extracted.
```

If you can name it (`handlePdfSave()`, `renderAgendaWeek()`, `validateUserInput()`), it deserves its own function.

### Rule 3 — Single Responsibility per file

```
One file = one concern. Period.
```

| ❌ Wrong | ✅ Right |
|----------|----------|
| `app.js` (PDF + Agenda + Habits + Auth + Courses) | `pdf-viewer.js`, `agenda.js`, `habits.js`, `auth.js`, `courses.js` |
| `server.py` (routes + DB + auth + config) | `routes/`, `models/`, `middleware/`, `config.py` |
| `styles.css` (everything) | `pdf.css`, `agenda.css`, `habits.css`, `layout.css` |

### Rule 4 — Architecture Review BEFORE code

Every SDD **Design** phase MUST answer these questions BEFORE writing code:

1. **Where does each new piece of code live?** (file path + module)
2. **Do we create new files or modify existing ones?** Justify WHY.
3. **Current file sizes**: How many lines does EACH file we touch have? If any > 500, the FIRST task MUST be "Refactor before adding features."
4. **Dependency graph**: What module depends on what? Minimize coupling.
5. **Export/API surface**: What does each module expose publicly? Keep it minimal.

### Rule 5 — No mixing concerns

```
Business logic ≠ Routing ≠ Persistence ≠ UI ≠ Configuration
```

| Layer | Responsibility | Example |
|-------|---------------|---------|
| **Presentation** | UI rendering, event handling | HTML, CSS, canvas drawing |
| **Domain/Business** | Business rules, calculations | Quiz scoring, session sorting |
| **Data/Persistence** | Storage, retrieval | API calls, SQL queries, localStorage |
| **Infrastructure** | Cross-cutting concerns | Auth, logging, config |

### Rule 6 — File naming convention by purpose

```
{concern}.{type}.{ext}   or   {type}/{concern}.{ext}
```

Examples:
- `pdf-viewer.js` (not `utils.js`)
- `agenda.routes.py` (not `routes.py` with everything)
- `auth.middleware.go` (not `middleware.go`)

### Rule 7 — Public API is explicit

Each module exposes a CLEAR, MINIMAL public API. What's internal stays private.

```javascript
// ✅ Good
const PdfViewer = {
    init(container) { /* ... */ },
    destroy(container) { /* ... */ },
    save(container) { /* ... */ },
};
// Internal functions are NOT exported
```

---

## SDD Integration

### During SPEC phase
Add a section **"Architecture Constraints"** listing:
- Current file sizes (all touched files)
- Whether any exceeds 500 lines
- Refactoring needed before this feature

### During DESIGN phase  — CRITICAL
The design document MUST include:
```markdown
## Module Architecture
- {file_path_1} → {responsibility} ({N} lines)
- {file_path_2} → {responsibility} ({N} lines)
- NEW: {file_path_3} → {responsibility}

## Dependency Graph
- {module_a} → {module_b} (imports/requires)
- {module_c} → {module_d}

## Refactoring Needed
- [ ] {file_x} exceeds 500 lines → split into {file_x1}, {file_x2}
- [ ] {function_y} exceeds 30 lines → extract helpers
```

If this section is missing, **DO NOT proceed to Tasks**.

### During TASKS phase
If the Design identified refactoring, the FIRST task MUST be:
```
1. [ ] Refactor: split {file_x} into modules ({list new files})
2. [ ] ... (feature tasks follow)
```

Do NOT implement features on top of broken structure.

### During APPLY phase
Before writing ANY code:
1. Check: are the files we're modifying under 500 lines? If not, STOP.
2. Check: are the functions we're modifying under 30 lines? If not, STOP.
3. Propose refactoring first, get approval, execute refactor, THEN add feature code.

### During VERIFY phase
Include architecture checks in the verification:
- [ ] No files exceed 500 lines
- [ ] No functions exceed 30 lines
- [ ] Single responsibility per file
- [ ] Concerns not mixed (UI ≠ logic ≠ data)
- [ ] Public API is explicit and minimal

---

## Commands

```bash
# Check file sizes in project
find . -type f \( -name "*.js" -o -name "*.py" -o -name "*.go" -o -name "*.ts" -o -name "*.css" \) | xargs wc -l | sort -rn | head -20

# Find functions longer than 30 lines (JS example)
awk '/function |=>/{f=$0; next} /^$/{f=""} f && NR,length(f)>30' app.js

# Check single file
wc -l path/to/file.js
```

---

## Checklist Before Any Code Change

```
Architecture Review:
- [ ] Files to modify: {list paths}
- [ ] Current sizes: {N lines each}
- [ ] Any > 500 lines? → Refactor FIRST
- [ ] Functions > 30 lines? → Extract FIRST
- [ ] New feature needs new file? → {justification}
- [ ] Module dependencies clear? → {diagram or list}
- [ ] Concerns separated? → {UI vs logic vs data}
```
