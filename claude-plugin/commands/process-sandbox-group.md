---
description: Process every currently-sandboxed file in the vault that resolves to a given system prompt name. Removes the sandbox marker on success, leaves the file sandboxed on failure. Continue-on-failure across files within the group.
---

# /remargin:process-sandbox-group <prompt-name>

Given a system-prompt name, process every sandboxed file in this vault that resolves to that prompt.

## Steps

1. **Enumerate currently-sandboxed files via activity.** Call `mcp__remargin__activity` with `path` = the vault scope (or the directory you're processing) and `pretty: true`. The result is a timestamp-sorted stream of events (comments, acks, edits, sandbox-adds, sandbox-removes) across **all identities**. Extract:
   - **Sandboxed set:** files whose most recent sandbox event is a `sandbox-add` with no later `sandbox-remove` by the same identity. This is the set to process.
   - **Recent context:** reactions on threads you're in, acks on your comments, comments addressed to others, edits, and signatures landed since your last action. Hold this for step 4 (per-file processing) — it's what your replies need to take into account. See remargin skill Critical rule 6.

   **Do not use `sandbox_list` for enumeration here.** It is caller-scoped and returns only the caller's own sandbox. In the typical agent-processing workflow the human user stages files for the agent — those won't appear in the agent's `sandbox_list`. `activity` sees stages by every identity, which is what this skill needs.

2. **Filter by resolved prompt.** For each file in the sandboxed set, call `mcp__remargin__prompt_resolve` and keep files whose resolved prompt name equals `<prompt-name>`. If the filtered list is empty, return a summary indicating no files matched and exit.

3. **Frame the work.** Look up the prompt body via `mcp__remargin__prompt_resolve` once (any matching file's resolution will do; they all resolve to the same prompt by construction). Treat the body as the current task definition.

4. **Process each file, sequentially — by invoking `/remargin:process-file`.** For each file in the filtered list:
   1. Invoke `/remargin:process-file <path>` via the Skill tool. That skill owns the per-file flow (activity check, prompt resolution, comment processing, body edits, inbound-pending verification, and per-file summary) — do not inline or duplicate its rules here. When relaying activity context from this group's step 1, hand the relevant slice to the agent before invoking the skill.
   2. On the per-file skill returning success (which now guarantees no inbound pendings remain on that file): call `mcp__remargin__sandbox_remove` with the file path.
   3. On the per-file skill returning failure or leaving inbound pendings: leave the sandbox marker in place. Record the failure. Carry on to the next file.

5. **Verify no inbound pendings remain across the group (defense-in-depth).** Call `mcp__remargin__query` with `pending: true` against the common ancestor directory of the processed files. The only pending entries should be replies you (the caller) posted, awaiting the other party's ack. Any **inbound** pending — a comment by an author other than you on a file you marked as successfully processed — is a contract violation by the per-file skill. Surface it loudly in the summary and reopen the affected file(s) before declaring done.

6. **Return a receipt, not a summary.** The chat message proves the round ran and shows the queue state. Nothing else.

   One summed receipt for the group, then a per-file `found`/`actions` table:

   ```
   document_reviewer — 3 files attempted

   | found   | 14 to me · 3 broadcast · 9 to others |
   | actions | replied 17 · acked 5 · body edits 4 · unsandboxed 2 |
   | after   | 0 inbound pending · 11 awaiting your ack · 1 left sandboxed |

   | file                     | found                | actions              |
   | ------------------------ | -------------------- | -------------------- |
   | notes/generated_types.md | 10 to me · 2 bcast   | replied 12 · acked 3 |
   | notes/schema_review.md   | 3 to me · 1 bcast    | replied 4 · acked 2  |
   | specs/emit_plan.md       | 1 to me              | replied 1 · edits 1  |
   ```

   With a single file attempted, emit the summed receipt alone. Counts only — no comment text, no ids. Drop any row whose counts are all zero. The `after` row carries step 5's inbound-pending verification; no file matched the prompt → a single line saying so.

   **Never restate, quote, summarize, or paraphrase comment or reply content in chat** — no per-comment tables, no "where I disagreed with you", no decision recaps, no list of document changes, and no relay of the per-file skill's own narration. If it was worth saying, it is already in the document.

   Two things stay in chat, because they are *not* in the document: **blockers** — every file left sandboxed by a failure, every inbound-pending leak found in step 5, and any op that was denied — and anything that **needs the owner's decision**. One line each, with why. Nothing else.

## Constraints

- Continue-on-failure within the group: a single file failure does not abort the rest.
- Same remargin skill rules as `/remargin:process-file`.
- Sandbox marker removal is per-file, after that file's processing succeeds — not at the end of the group. Partial progress is preserved.
- The system prompt is fixed for the duration of this invocation. Files outside the group are not touched.
