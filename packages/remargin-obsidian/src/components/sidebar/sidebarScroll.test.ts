import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

// Guards the sidebar's single scroll container against the flexbox
// `min-height: auto` trap. A flex item will not shrink below its content
// height unless `min-h-0` is set, so without it `flex-1` never caps the
// ScrollArea: the Radix root grows to the full comment list, scrollHeight
// equals clientHeight, no thumb is usable even under `type="always"`, and
// Obsidian's outer pane scrolls instead of the panel.
//
// Asserted against the source text rather than a rendered tree because the
// bug lives purely in the class list; rendering it would need a real layout
// engine (jsdom computes no box sizes) to observe the same thing.
const here = dirname(fileURLToPath(import.meta.url));
const shellSource = readFileSync(join(here, "SidebarShell.tsx"), "utf8");
const scrollAreaSource = readFileSync(join(here, "..", "ui", "scroll-area.tsx"), "utf8");

/** The class list of the `<ScrollArea>` element in `SidebarShell`. */
function scrollAreaClasses(): string {
  const match = shellSource.match(/<ScrollArea className="([^"]+)"/);
  assert.ok(match, "SidebarShell must render a <ScrollArea> with a className");
  return match[1];
}

describe("sidebar scroll container", () => {
  it("sets min-h-0 so flex-1 can actually cap its height", () => {
    assert.match(
      scrollAreaClasses(),
      /\bmin-h-0\b/,
      "without min-h-0 the panel grows to its content and never scrolls itself"
    );
  });

  it("keeps min-w-0 for the same reason on the horizontal axis", () => {
    assert.match(scrollAreaClasses(), /\bmin-w-0\b/);
  });

  it("still stretches to the available space", () => {
    assert.match(scrollAreaClasses(), /\bflex-1\b/);
  });

  it("lives inside a full-height flex column, which is what flex-1 measures", () => {
    assert.ok(
      shellSource.includes('className="flex flex-col h-full min-w-0 bg-bg-primary"'),
      "the shell root must stay a full-height flex column for min-h-0 to matter"
    );
  });

  it("renders an always-visible scrollbar, so a capped height shows a thumb", () => {
    assert.match(scrollAreaSource, /type="always"/);
  });
});
