---
description: Process a folder driven by activity. First reads the full activity delta across ALL identities to build awareness, then acts only on items pending to this identity or open/unassigned. Groups by resolved system prompt and spawns one subagent per group. Does NOT touch sandbox markers.
---

# /remargin:process-folder <path>

Activity-driven folder processing. Awareness comes first — the full activity delta across every identity — then action focuses on what is pending to this identity or open.

## Steps

1. **Build awareness from activity.** Call `mcp__remargin__activity` with `path` = the folder and `pretty: true`. This delta — across ALL identities (the user's edits and comments, other agents' activity, acks, sandbox-adds) — is BOTH the file-selection input and the context you hold before acting. Read all of it before touching anything. Do not drive selection from a pending-only query — pending-to-me is one slice and skips the rest of the picture (the user's edits, other agents' replies, broadcasts). This ordering is the core of the command.

2. **Determine the action set.** Within that awareness, the files you act on are those carrying an item pending to THIS identity OR open/unassigned — directed-to-me plus unacked broadcasts. Files surfaced by activity with no actionable item for you are read for context but NOT mutated.

3. **Group by resolved prompt.** For each file in the action set, call `mcp__remargin__prompt_resolve` and bucket by resolved prompt name. If the action set is empty, return a summary saying what activity was seen and that nothing was actionable, then exit.

4. **Process each group via a subagent — sequentially.** For each prompt name with at least one actionable file:
   1. Spawn a subagent via the `Agent` tool with `subagent_type: "general-purpose"`. Instruct it to process exactly the files in this group under the resolved system prompt body (included inline), following the `/remargin:process-file` flow per file.
   2. Wait for completion. Capture its summary.
   3. Move to the next group. Do NOT run groups in parallel — sequential subagents preserve the user's ability to follow what's happening.

5. **Do NOT touch sandbox markers.** This command is activity-driven, not sandbox-driven — it neither requires nor clears sandbox state. This is the key behavioral difference from `/remargin:process-sandbox`, which removes markers on success.

6. **Return a receipt, not a summary.** The chat message proves the round ran and shows the queue state. Nothing else.

   One file acted on → one receipt:

   ```
   notes/generated_types.md — processed

   | found   | 10 to me · 2 broadcast · 5 to others |
   | actions | replied 12 · acked 3 · body edits 2 · issues filed 1 |
   | after   | 0 inbound pending · 6 awaiting your ack |
   ```

   Several files acted on → one summed receipt for the run, then a per-file `found`/`actions` table:

   ```
   notes/ — processed · 3 of 9 files · 2 groups

   | found   | 14 to me · 3 broadcast · 9 to others |
   | actions | replied 17 · acked 5 · body edits 4 |
   | after   | 0 inbound pending · 11 awaiting your ack |

   | file                     | found                | actions              |
   | ------------------------ | -------------------- | -------------------- |
   | notes/generated_types.md | 10 to me · 2 bcast   | replied 12 · acked 3 |
   | notes/schema_review.md   | 3 to me · 1 bcast    | replied 4 · acked 2  |
   | specs/emit_plan.md       | 1 to me              | replied 1 · edits 1  |
   ```

   Counts only — no comment text, no ids. Drop any row whose counts are all zero. Files activity surfaced but that carried nothing actionable are the difference between the two file counts in the header line, never a list. Nothing actionable at all → a single line saying so.

   **Never restate, quote, summarize, or paraphrase comment or reply content in chat** — no per-comment tables, no "where I disagreed with you", no decision recaps, no list of document changes, and no relay of a subagent's own narration. If it was worth saying, it is already in the document.

   Two things stay in chat, because they are *not* in the document: **blockers** (an op that was denied, or a comment no subagent could act on) and anything that **needs the owner's decision**. One line each, with why. Nothing else.

## Constraints

- Awareness = activity (full delta, all identities); action focus = pending-to-me + open. That ordering is the whole point of this command.
- One subagent per group, sequential. Context isolation comes from the subagent boundary; each subagent receives its prompt body inline and must not consult any other system prompt.
- No sandbox markers touched (neither required nor cleared) — the difference from `process-sandbox`.
- Same remargin skill rules as `/remargin:process-file` apply inside every subagent (MCP over CLI, batch for N replies, ack only after the work is done, never delete other participants' comments).
- Files with no actionable item for the caller are read for context only — never mutated.
