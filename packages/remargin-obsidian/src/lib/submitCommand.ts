/**
 * Pure composition helpers for the sandbox Submit flow. The plugin
 * writes each group's composed prompt to a temp file, then launches a
 * terminal running one `cat <promptfile> | <runner>` per group,
 * chained with `;` (sequential, continue-on-failure).
 */

/** Wrap `s` in single quotes, escaping embedded single quotes. */
export function shellQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/** Default runner used when the resolved prompt has no `runner:`. */
export function defaultRunner(claudePath: string, remarginPath: string): string {
  const claude = claudePath.trim() || "claude";
  const mcpConfig = JSON.stringify({
    mcpServers: { remargin: { command: remarginPath.trim() || "remargin", args: ["mcp", "run"] } },
  });
  return `${shellQuote(claude)} -p --permission-mode auto --allowedTools "mcp__remargin__*" --mcp-config ${shellQuote(mcpConfig)}`;
}

/**
 * Inline prompt body: resolved prompt + file list + marker-cleanup
 * instruction. With no completion hook in the plugin, the launched
 * agent owns clearing the sandbox markers of the files it processed.
 */
export function composeInlinePrompt(prompt: string, files: string[]): string {
  const fileList = files.length > 0 ? `\n\nFiles:\n${files.join("\n")}` : "";
  return (
    `${prompt}${fileList}\n\n` +
    "When you finish processing a file successfully, remove its sandbox marker\n" +
    "(remargin MCP tool `sandbox_remove`, or `remargin sandbox remove <file>`).\n" +
    "Leave the marker in place for any file you could not process.\n"
  );
}

/** One `cat <promptfile> | <runner>` per group, chained with `;`. */
export function buildSubmitShellLine(entries: { promptFile: string; runner: string }[]): string {
  return entries.map((e) => `cat ${shellQuote(e.promptFile)} | ${e.runner}`).join("; ");
}

/** Filesystem-safe slug for a group's temp prompt-file basename. */
export function promptFileSlug(name: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug.length > 0 ? slug : "default";
}
