import { strict as assert } from "node:assert";
import { describe, it } from "node:test";
import { Mail } from "lucide-react";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Collapsible } from "../ui/collapsible.tsx";
import { SectionHeader } from "./SectionHeader.tsx";
import { ViewToggle } from "./ViewToggle.tsx";

// SSR-only render — the trigger needs a Radix Collapsible root above it.

const noop = (): void => {
  /* test-only no-op */
};

function render(variant: "default" | "sandbox"): string {
  return renderToStaticMarkup(
    createElement(
      Collapsible,
      { open: true },
      createElement(SectionHeader, {
        icon: Mail,
        title: "Inbox",
        badge: 3,
        open: true,
        variant,
        actions: createElement(ViewToggle, { value: "flat", onChange: noop }),
      })
    )
  );
}

/** The HTML content model forbids interactive content inside <button>. */
function assertNoNestedButtons(html: string): void {
  let depth = 0;
  for (const match of html.matchAll(/<(\/?)button\b/g)) {
    if (match[1] === "/") {
      depth -= 1;
    } else {
      assert.equal(depth, 0, `<button> nested inside <button> at index ${match.index}: ${html}`);
      depth += 1;
    }
  }
  assert.equal(depth, 0, `unbalanced <button> tags: ${html}`);
}

describe("SectionHeader — actions render outside the trigger button", () => {
  for (const variant of ["default", "sandbox"] as const) {
    it(`${variant} variant: no <button> has a <button> descendant`, () => {
      const html = render(variant);
      // Sanity: both the trigger and the ViewToggle buttons rendered.
      const buttonCount = [...html.matchAll(/<button\b/g)].length;
      assert.equal(buttonCount, 3, `expected trigger + 2 toggle buttons, got: ${html}`);
      assertNoNestedButtons(html);
    });

    it(`${variant} variant: no click-shield wrapper remains`, () => {
      const html = render(variant);
      assert.ok(!html.includes('role="presentation"'), `expected no shield wrapper, got: ${html}`);
    });
  }
});
