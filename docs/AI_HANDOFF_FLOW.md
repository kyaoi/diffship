# AI Handoff Flow

This document explains the **practical end-to-end flow** for using diffship with ChatGPT, Claude, Codex, or similar generative AI tools.

It answers three questions:

1. what the user should send to AI
2. what output format the user should request from AI
3. how the user should use the AI result afterward

This is a **human workflow guide**, not the formal product contract.

For the formal contracts, see:

- `docs/SPEC_V1.md`
- `docs/BUNDLE_FORMAT.md`
- `docs/PATCH_BUNDLE_FORMAT.md`

---

## 1. The overall flow

The intended flow is:

1. collect local changes into a handoff bundle
2. inspect the bundle before sharing it
3. give the bundle plus a clear task to AI
4. receive AI output in a format appropriate for the task
5. review the result
6. if the AI produced an ops-compatible patch bundle, run `diffship loop`
7. if the AI produced review notes or patch text only, apply it manually or hand it to a local coding agent

In short:

```text
local Git changes
  -> diffship build
  -> diffship preview / compare
  -> send to AI
  -> AI response
  -> user review
  -> diffship loop   (if patch bundle)
  -> or manual/local-agent application   (if text-only response)
```

---

## 2. What the user should send to AI

### 2.1 The preferred payload

The preferred payload is the generated handoff bundle:

- directory bundle, or
- zip bundle created by `diffship build --zip`

The handoff bundle already contains the structure AI needs:

- `HANDOFF.md`
- `parts/part_XX.patch`
- optional `attachments.zip`
- optional `excluded.md`
- optional `secrets.md`
- optional `plan.toml`

### 2.2 Minimum recommended context

When asking AI to work on your changes, provide:

1. the handoff bundle
2. a short goal statement
3. explicit constraints
4. the expected output format

Example context to include outside the bundle:

- what you want changed
- what must not change
- whether this is review-only or implementation
- whether the spec is allowed to change
- whether the AI should produce prose only, patch text, or a patch bundle

### 2.3 What not to send unless necessary

Avoid sending:

- the whole repository snapshot
- generated directories such as `target/`
- unrelated large assets
- secrets or credentials
- noisy diffs not relevant to the task

Use `.diffshipignore`, `--include`, and `--exclude` to narrow the handoff.

---

## 3. What AI should read first

When an AI receives a diffship handoff bundle, the expected reading order is:

1. `HANDOFF.md`
2. the first patch part in `parts/`
3. remaining parts in order
4. `excluded.md`, if present
5. `attachments.zip`, if needed
6. `secrets.md`, if present

Why:

- `HANDOFF.md` explains scope, reading order, and part mapping
- patch parts are the primary source of code changes
- exclusions and attachments are secondary context

If you are instructing AI explicitly, tell it:

```text
Read HANDOFF.md first, then read the patch parts in order.
Treat HANDOFF.md as the map and the patch parts as the primary source of truth.
```

---

## 4. Output formats to request from AI

The right output format depends on the task.

### 4.1 Review-only format

Use this when you want analysis without code changes.

Ask for:

- findings first
- grouped by severity
- file/path references where possible
- missing tests or risks
- optional minimal patch suggestions

Recommended format:

```text
Output format:
  - Findings grouped into Must / Should / Nice-to-have
  - Each finding should include the affected file/path and the reason
  - If helpful, add a minimal patch snippet
  - Do not rewrite the whole repository
```

This is the safest default.

### 4.2 Spec/doc planning format

Use this when the main goal is alignment, inventory, or planning.

Ask for:

- implementation status summary
- mismatches between code/tests/docs
- minimal doc changes
- next-task proposal with completion criteria

Recommended format:

```text
Output format:
  1. Current status summary
  2. Mismatches between implementation, tests, and docs
  3. Minimal doc/spec updates needed
  4. Next tasks with priority and completion conditions
```

### 4.3 Text patch / file-edit format

Use this when the AI cannot generate an ops-compatible patch bundle, but you still want concrete edits.

Ask for:

- unified diffs, or
- file-by-file edits, or
- patch snippets only for touched files

Recommended format:

```text
Output format:
  - Provide unified diffs or file-by-file edit blocks
  - Touch the minimum files needed
  - Update docs/tests together if behavior changes
  - Do not include generated artifacts
```

This format is suitable for:

- ChatGPT in text mode
- review threads
- handoff to another local coding agent

### 4.4 Ops-compatible patch bundle format

Use this only if the AI environment can actually produce a patch bundle directory or zip matching `docs/PATCH_BUNDLE_FORMAT.md`.

Ask for:

- patch bundle output, not prose-first output
- deterministic file ordering
- valid `manifest.yaml`
- patch files under `changes/`
- optional `summary.md`, `commit_message.txt`, `tasks/`

Recommended instruction:

```text
Output format:
  - Produce a patch bundle compatible with docs/PATCH_BUNDLE_FORMAT.md
  - Include manifest.yaml with correct base_commit and touched_files
  - Put file patches under changes/
  - Do not invent extra bundle files unless they follow the contract
  - If you cannot produce a valid patch bundle, say so and return review notes or unified diffs instead
```

This is the format that can flow directly into:

```bash
diffship loop ./patch-bundle.zip
```

---

## 5. Practical request templates

### 5.1 Review-only request

```text
I am attaching a diffship handoff bundle.

Task:
  Review the proposed changes only. Do not implement anything.

Read order:
  - Read HANDOFF.md first
  - Then read parts/part_XX.patch in order

Focus:
  - correctness
  - regressions
  - missing tests
  - doc/spec mismatches

Output format:
  - Findings grouped into Must / Should / Nice-to-have
  - Include affected file/path references where possible
  - Add small patch suggestions only if helpful
```

### 5.2 Implementation planning request

```text
I am attaching a diffship handoff bundle.

Task:
  Inspect the current implementation and propose the minimum changes needed.

Constraints:
  - Read HANDOFF.md first
  - Keep changes small and reviewable
  - If behavior changes, update docs/tests/traceability together
  - Do not do unrelated refactors

Output format:
  1. Current status summary
  2. Required changes
  3. Tests to add or update
  4. Minimal implementation plan
```

### 5.3 Text patch request

```text
I am attaching a diffship handoff bundle.

Task:
  Implement the requested change and return unified diffs only.

Constraints:
  - Read HANDOFF.md first
  - Touch the minimum files needed
  - Keep docs/tests in sync with behavior
  - Do not include generated files

Output format:
  - Unified diffs only
  - If you cannot finish safely, explain blockers instead of guessing
```

### 5.4 Patch-bundle request

```text
I am attaching a diffship handoff bundle.

Task:
  Implement the requested change and return an ops-compatible patch bundle.

Constraints:
  - Follow docs/PATCH_BUNDLE_FORMAT.md
  - Use the correct base_commit
  - Keep output deterministic
  - Include only contract-compliant bundle files

Fallback:
  - If you cannot generate a valid patch bundle, return review notes or unified diffs instead
```

---

## 6. How the user should use the AI result

### 6.1 If the AI returned review findings

The user should:

1. read the findings
2. decide which items are real issues
3. fix them locally, or
4. send the findings to a coding agent that can edit the repo

This path does **not** use `diffship loop` directly.

### 6.2 If the AI returned text patches or file edits

The user should:

1. inspect the proposed diffs
2. apply them manually or with a local coding agent
3. run local checks
4. optionally create a new handoff bundle if another review round is needed

This is useful when the AI can reason well but cannot produce a valid patch bundle artifact.

### 6.3 If the AI returned a patch bundle

The user should:

1. save the patch bundle locally
2. inspect the bundle if needed
3. run:

```bash
diffship loop ./patch-bundle.zip
```

4. if promotion is blocked, decide whether to rerun with:

```bash
diffship loop ./patch-bundle.zip --ack-secrets
diffship loop ./patch-bundle.zip --ack-tasks
```

5. if verification fails, inspect the run and reprompt bundle:

```bash
diffship status
diffship runs
diffship pack-fix --run-id <run-id>
```

---

## 7. Suggested user flow

For typical day-to-day use, this is the recommended sequence.

### Step 1: build a focused handoff bundle

```bash
diffship build --include 'src/*.rs' --include 'docs/*.md' --exclude 'src/generated.rs'
```

### Step 2: inspect before sharing

```bash
diffship preview ./diffship_2026-03-07_1118_abcdef1 --list
diffship preview ./diffship_2026-03-07_1118_abcdef1 --part part_01.patch
```

Optional reproducibility check:

```bash
diffship compare ./bundle_a ./bundle_b --json
```

### Step 3: send to AI with explicit instructions

Tell the AI:

- what the goal is
- what it must not change
- what output format you want
- whether you want review-only, diffs, or a patch bundle

### Step 4: process the AI result

- findings only -> review and act locally
- diffs only -> apply locally or with another agent
- patch bundle -> run `diffship loop`

### Step 5: verify and promote

If you received a patch bundle:

```bash
diffship loop ./patch-bundle.zip
```

### Step 6: if it fails, iterate with evidence

Use run logs and reprompt artifacts:

```bash
diffship status
diffship runs
diffship pack-fix --run-id <run-id>
```

Then send the new evidence back to AI.

---

## 8. Recommended defaults

If you are unsure, use these defaults:

- start with **review-only**
- keep the bundle narrow
- tell AI to read `HANDOFF.md` first
- ask for findings before code
- use unified diffs if patch-bundle generation is not guaranteed
- use `diffship loop` only for contract-compliant patch bundles

---

## 9. Relationship to other docs

- `docs/AI_WORKFLOW.md`
  - high-level rules for working with AI in this repository
- `docs/USAGE_GUIDE.md`
  - command-oriented guide for using diffship
- `docs/HANDOFF_TEMPLATE.md`
  - expected structure of generated `HANDOFF.md`
- `docs/PATCH_BUNDLE_FORMAT.md`
  - required structure for ops-compatible patch bundles

Use this file when you want the **human operational flow** for talking to AI from diffship output to final application.
