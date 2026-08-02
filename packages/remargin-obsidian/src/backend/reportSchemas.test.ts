import { strict as assert } from "node:assert";
import { describe, it } from "node:test";
import {
  DoctorReport$Schema,
  ReplaceFileOutcome$Schema,
  ReplaceReport$Schema,
  SandboxListEntry$Schema,
  SandboxRemoveReport$Schema,
} from "@/generated";

// Every payload below is REAL `remargin <cmd> --json` stdout captured from the
// CLI (not hand-authored). The io layer decorates every top-level response with
// an injected `elapsed_ms`, so a schema that models a whole payload has to admit
// that one key while staying strict about every other unknown key; a schema that
// models a fragment nested inside a payload never sees it.

const DOCTOR = `{
  "elapsed_ms": 24,
  "findings": [],
  "hook_installed": true,
  "project_settings_file": "/home/eduardoburgos/src/tixena/remargin/.claude/settings.json",
  "session_guard_installed": true,
  "user_settings_file": "/home/eduardoburgos/.claude/settings.json"
}`;

const REPLACE = `{
  "dry_run": false,
  "elapsed_ms": 2,
  "files": [ { "changed": true, "path": "doc.md", "replacements": 1 } ],
  "files_changed": 1,
  "files_failed": 0,
  "total_replacements": 1
}`;

const SANDBOX_REMOVE = `{
  "elapsed_ms": 2, "failed": [], "removed": ["doc.md"], "skipped": []
}`;

// One `sandbox list` row, captured from the same run — a nested element, so the
// envelope's elapsed_ms never reaches it.
const SANDBOX_LIST_ENTRY = `{ "path": "doc.md", "since": "2026-08-02T14:10:26.961832093+00:00" }`;

describe("report schemas — live --json payloads parse whole", () => {
  it("doctor: parses the report the CLI actually emits", () => {
    const report = DoctorReport$Schema.parse(JSON.parse(DOCTOR));
    assert.equal(report.hook_installed, true);
    assert.equal(report.findings.length, 0);
    assert.equal(report.elapsed_ms, 24);
  });

  it("replace: parses the report with its nested file outcomes", () => {
    const report = ReplaceReport$Schema.parse(JSON.parse(REPLACE));
    assert.equal(report.total_replacements, 1);
    assert.equal(report.files[0].path, "doc.md");
    assert.equal(report.elapsed_ms, 2);
  });

  it("sandbox remove: parses the report", () => {
    const report = SandboxRemoveReport$Schema.parse(JSON.parse(SANDBOX_REMOVE));
    assert.deepStrictEqual(report.removed, ["doc.md"]);
    assert.equal(report.elapsed_ms, 2);
  });

  it("elapsed_ms is optional, so a payload without it still parses", () => {
    const payload = JSON.parse(DOCTOR);
    delete payload.elapsed_ms;
    const report = DoctorReport$Schema.parse(payload);
    assert.equal(report.elapsed_ms, undefined);
  });
});

describe("report schemas — strictness retained", () => {
  it("doctor: rejects an unknown key that is not elapsed_ms", () => {
    const bad = { ...JSON.parse(DOCTOR), bogus_field: 1 };
    assert.equal(DoctorReport$Schema.safeParse(bad).success, false);
  });

  it("replace: rejects an unknown key that is not elapsed_ms", () => {
    const bad = { ...JSON.parse(REPLACE), bogus_field: 1 };
    assert.equal(ReplaceReport$Schema.safeParse(bad).success, false);
  });

  it("sandbox remove: rejects an unknown key that is not elapsed_ms", () => {
    const bad = { ...JSON.parse(SANDBOX_REMOVE), bogus_field: 1 };
    assert.equal(SandboxRemoveReport$Schema.safeParse(bad).success, false);
  });

  it("doctor: rejects a non-numeric elapsed_ms", () => {
    const bad = { ...JSON.parse(DOCTOR), elapsed_ms: "24" };
    assert.equal(DoctorReport$Schema.safeParse(bad).success, false);
  });
});

describe("nested schemas — no elapsed_ms member", () => {
  it("replace file outcome parses on its own but rejects elapsed_ms", () => {
    const element = JSON.parse(REPLACE).files[0];
    assert.equal(ReplaceFileOutcome$Schema.parse(element).replacements, 1);
    assert.equal(ReplaceFileOutcome$Schema.safeParse({ ...element, elapsed_ms: 2 }).success, false);
  });

  it("sandbox list entry parses on its own but rejects elapsed_ms", () => {
    const element = JSON.parse(SANDBOX_LIST_ENTRY);
    assert.equal(SandboxListEntry$Schema.parse(element).path, "doc.md");
    assert.equal(SandboxListEntry$Schema.safeParse({ ...element, elapsed_ms: 2 }).success, false);
  });
});
