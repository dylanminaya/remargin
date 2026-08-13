---
description: Process every currently-sandboxed file in the vault. Groups by resolved system prompt and spawns one subagent per group so each group runs in its own fresh context — no system-prompt mixing across groups.
---

# /remargin:process-sandbox

Vault-wide sandbox processing. Each prompt group runs in an isolated subagent context so the system prompts don't bleed across groups.

## Steps

1. **Enumerate currently-sandboxed files via activity.** Call `mcp__remargin__activity` with `path` = the vault root and `pretty: true`. The result is a timestamp-sorted stream of events across **all identities**. A file is currently sandboxed iff its most recent sandbox event is a `sandbox-add` with no later `sandbox-remove` by the same identity. Collect that set.

   **Do not use `sandbox_list` for enumeration here.** It is caller-scoped and returns only the caller's own sandbox. In the typical agent-processing workflow the human user stages files for the agent — those won't appear in the agent's `sandbox_list`. `activity` sees stages by every identity.

2. **Group by prompt.** For each file, call `mcp__remargin__prompt_resolve` and bucket by the resolved prompt name. If no files are sandboxed, emit step 4's nothing-sandboxed single line and exit.

3. **Process each group via a subagent — sequentially.** For each prompt name with at least one sandboxed file:
   1. Spawn a subagent via the `Agent` tool with `subagent_type: "general-purpose"`. The prompt for the subagent: instruct it to process exactly the files in this group, under the resolved system prompt body, following the same flow as `/remargin:process-sandbox-group <prompt-name>`. Include the prompt body inline so the subagent has full context.
   2. Wait for the subagent to complete. Capture its counts and blockers for the receipt — never its prose.
   3. Move to the next group. Do NOT do groups in parallel — sequential subagents preserve the user's ability to follow what's happening.

4. **Return a receipt, not a summary.** The chat message proves the round ran and shows the queue state. Nothing else.

   One summed vault-level receipt, then a per-file `found`/`actions` table:

   ```
   vault — sandbox processed · 3 files · 2 groups

   | found   | 14 to me · 3 broadcast · 9 to others |
   | actions | replied 17 · acked 5 · body edits 4 · unsandboxed 2 |
   | after   | 0 inbound pending · 11 awaiting your ack · 1 left sandboxed |

   | file                     | found                | actions              |
   | ------------------------ | -------------------- | -------------------- |
   | notes/generated_types.md | 10 to me · 2 bcast   | replied 12 · acked 3 |
   | notes/schema_review.md   | 3 to me · 1 bcast    | replied 4 · acked 2  |
   | specs/emit_plan.md       | 1 to me              | replied 1 · edits 1  |
   ```

   With a single file processed, emit the summed receipt alone. Counts only — no comment text, no ids. Drop any row whose counts are all zero. Nothing sandboxed → a single line saying so.

   **Never restate, quote, summarize, or paraphrase comment or reply content in chat** — no per-comment tables, no "where I disagreed with you", no decision recaps, no list of document changes, and no relay of a subagent's own narration. If it was worth saying, it is already in the document.

   Two things stay in chat, because they are *not* in the document: **blockers** — every file left sandboxed by a failure, plus any op that was denied — and anything that **needs the owner's decision**. One line each, with why. Nothing else.

## Constraints

- One subagent per group, sequential. Context isolation comes from the subagent boundary, not from process boundaries.
- Each subagent receives the prompt body inline; it must not consult any other system prompt.
- Sandbox marker removal happens inside each subagent on per-file success (same rule as `/remargin:process-sandbox-group`). Failures leave files sandboxed.
- Continue-on-failure across groups: a failure in group A does not stop group B.
- Same remargin skill rules as `/remargin:process-file` apply inside every subagent (MCP > CLI, batch for N replies, ack only after the work is done, etc.).
- Files outside the sandbox are not touched.
