---
name: 'step-03a-execute-review'
description: 'Autonomous execution loop - automate and code review'
nextStep: './step-03b-execute-finish.md'
scriptsDir: '../scripts/story-automator'
outputFile: '{output_folder}/story-automator/orchestration-{epic_id}-{timestamp}.md'
retryStrategy: '../data/retry-fallback-strategy.md'
reviewLoop: '../data/code-review-loop.md'
---

# Step 3a: Execute Review Phase

**Goal:** Run automate (guardrails) and code review loop for the current story.
**Interaction mode:** Deterministic autonomous execution.

---

## Prerequisites

- Step 3 completed (create-story and dev-story done)
- State document updated with current story progress

Set: `scripts="{scriptsDir}"`

---

## Story Loop (Continue from Step 3)

### C. Automate (Guardrails)
*Skip if `overrides.skipAutomate`*

**Apply retry/fallback pattern from `{retryStrategy}`:** Non-blocking, but still retry on failure.

```bash
# --command required (see Spawn Pattern in step-03)
resolve_agent_for_task "auto" "$state_file" "{story_id}"
if should_apply_primary_model "$current_agent"; then
  built_cmd=$("$scripts" tmux-wrapper build-cmd auto {story_id} --agent "$current_agent" --model "$primary_model" --state-file "$state_file")
else
  built_cmd=$("$scripts" tmux-wrapper build-cmd auto {story_id} --agent "$current_agent" --state-file "$state_file")
fi
session=$("$scripts" tmux-wrapper spawn auto {epic} {story_id} \
  --agent "$current_agent" \
  --command "$built_cmd")
result=$("$scripts" monitor-session "$session" --json --agent "$current_agent")
"$scripts" tmux-wrapper kill "$session"
```

- SUCCESS:
  ```bash
  # Update Story Progress: mark automate done
  tmp_state=$(mktemp)
  sed "s/^| ${story_id} |.*$/| ${story_id} | done | done | done | - | - | in-progress |/" "{outputFile}" > "$tmp_state" && mv "$tmp_state" "{outputFile}"
  ```
  Display: `[story {N}/{total}] automate -> done`
  → proceed to C2
- FAILURE → retry up to 3 attempts (non-blocking, so fewer retries), then log warning:
  ```bash
  # Update Story Progress: mark automate skipped
  tmp_state=$(mktemp)
  sed "s/^| ${story_id} |.*$/| ${story_id} | done | done | skip | - | - | in-progress |/" "{outputFile}" > "$tmp_state" && mv "$tmp_state" "{outputFile}"
  ```
  Display: `[story {N}/{total}] automate -> skip (non-blocking)`
  → proceed to C2

### C2. File-List ⇄ git reconciliation (issue #18 — MANDATORY, runs whether automate ran or was skipped)

The single most repeated review finding across Epic 1 (and recurring in Epic 2) was a story `### File List` that omitted files the dev-story / automate step created — especially `*_qa_e2e.rs` suites, `tests/test-summary.md`, and automator logs — plus stale test-count claims. Fix it **before** handing off to review so the review never re-raises it.

The orchestrator (which has repo access between sub-sessions) runs:

```bash
# Every tracked-but-modified and untracked file under the story's source/test trees.
changed=$(git -C "{project-root}" status --porcelain -- '*.rs' '*.slint' 'tests/' '**/tests/' '*test-summary.md' \
  | sed -E 's/^.{3}//')
```

Then, for the current story file (`{story_file}`):
1. For each path in `$changed`, verify it appears verbatim in the story's `### File List`. Append any missing path with a one-line role note (mark `(NEW)`/`(M)`), preserving the existing list.
2. Refresh any test-count claim in **Dev Agent Record / Completion Notes / Change Log** to the real number (`cargo test --all` summary), or add a Change Log line if the automate step added suites (per 1.8's review precedent).
3. Do **not** silently mark `[x]` for a test that does not exist on disk — if a task claims a test, confirm the file + test name are present before leaving the checkbox checked.

- DONE → Display: `[story {N}/{total}] file-list-sync -> done` → proceed to D
- If `$changed` is empty (nothing to add) → proceed to D

This is a structural gate: the code-review step (D) must inherit a File List that already equals `git`.

### D. Code Review Loop

**See `{reviewLoop}` for complete script-based review cycle with v2.3 per-task agent configuration.**

**MANDATORY log-summary contract (every review cycle):**
- Run a single grep/regex pass over review output first.
- Return only compact fields to parent flow: `next_action`, `confidence`, `error_class`, `issues_count`, `top_issues`.
- Do not carry full log payloads forward unless escalation requires raw evidence.

```bash
review_log=$(echo "$result" | jq -r '.output_file')
review_focus=$(grep -nE "SUCCESS|FAIL|ERROR|CRITICAL|WARN|RETRY|ESCALATE|ISSUE" "$review_log" | head -n 120)
if [ -z "$review_focus" ]; then
  review_focus=$(tail -n 120 "$review_log")
fi

# Compact subprocess-style summary contract for parent flow
review_summary=$("$scripts" orchestrator-helper parse-output "$review_log" review --state-file "$state_file" | jq -c '
  {
    next_action: (.next_action // "retry"),
    confidence: (.confidence // 0),
    error_class: (.error_class // "unknown"),
    issues_count: ((.issues // []) | length),
    top_issues: ((.issues // [])[:3])
  }
')
```

Key points:
- Up to 5 cycles using `story-automator tmux-wrapper spawn review` + `story-automator monitor-session`
- **Agent:** Uses per-task config from state document (`resolve_agent_for_task "review"`)
- **Verification:** Uses `--workflow review --story-key` for sprint-status verification
- **States:** `completed` (verified):
  ```bash
  # Update Story Progress: mark code-review done
  tmp_state=$(mktemp)
  sed "s/^| ${story_id} |.*$/| ${story_id} | done | done | done | done | - | in-progress |/" "{outputFile}" > "$tmp_state" && mv "$tmp_state" "{outputFile}"
  ```
  Display: `[story {N}/{total}] review -> done`
  → E | `incomplete` → count as failed attempt, retry until maxCycles, then CRITICAL escalate (Trigger #8)
- Exit loop when sprint-status shows "done"
- If `review_summary.next_action` is ambiguous, ask one clarifying question before escalating.

---

## Auto-Proceed to Finalization

Display: "**Code review complete. Proceeding to finalize commits and status checks...**"

```bash
"$scripts" orchestrator-helper state-update "{outputFile}" \
  --set currentStep=step-03b-execute-finish \
  --set lastUpdated="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "- **[$(date -u +%Y-%m-%dT%H:%M:%SZ)]** Code review complete, proceeding to finalization" >> "{outputFile}"
```

---

## Then
→ Immediately load and execute `{nextStep}`
