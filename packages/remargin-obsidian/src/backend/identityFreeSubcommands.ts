/**
 * Subcommands that reject identity flags (identity-free machine/tree
 * reads). Contract: a wrong entry fails LOUD — the CLI errors on an
 * unexpected argument — never as a silent identity switch.
 *
 * Every other subcommand acts on behalf of the user and gets the
 * identity flags built from plugin settings forwarded by default.
 *
 * Kept in its own file so unit tests can import it without pulling in
 * `RemarginBackend.ts`, whose constructor uses TypeScript parameter
 * properties the test runner's strip-only loader cannot parse.
 */
export const IDENTITY_FREE_SUBCOMMANDS = new Set(["resolve-mode", "obsidian", "registry"]);

/**
 * The exec gate: forward identity unless the subcommand is identity-free
 * or the invocation is a bare-flag probe like `--version`.
 */
export function acceptsIdentity(subcommand: string | undefined): boolean {
  return (
    subcommand !== undefined &&
    !subcommand.startsWith("-") &&
    !IDENTITY_FREE_SUBCOMMANDS.has(subcommand)
  );
}
