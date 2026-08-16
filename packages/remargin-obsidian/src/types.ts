import type { UpdateCheckState } from "./lib/githubReleases";

/** "flat" renders a single-level list; "tree" groups files by directory. */
export type ViewMode = "flat" | "tree";

/**
 * Inbox filter mode. Each value maps to one server-side `remargin query`
 * narrowing (see `inboxFilterQueryOpts`): comments waiting on me, my own
 * still-pending comments, unacked broadcasts, the broad pending
 * predicate, and everything.
 */
export type InboxFilter = "for-me" | "from-me" | "unassigned" | "pending" | "all";

export interface RemarginSettings {
  remarginPath: string;
  claudePath: string;
  /**
   * Terminal emulator argv prefix for sandbox Submit, e.g. `ptyxis --`
   * or `kitty -e`. Empty = auto-detect per OS.
   */
  terminalCommand: string;
  workingDirectory: string;
  identityMode: "config" | "manual";
  configFilePath: string;
  authorName: string;
  keyFilePath: string;
  sidebarSide: "left" | "right";
  /** Per-section view mode, persisted across sessions (UI task 26). */
  sandboxView: ViewMode;
  inboxView: ViewMode;
  /** Last inbox filter mode picked in the sidebar dropdown. */
  inboxFilter: InboxFilter;
  /**
   * User toggle for the GitHub-releases update probe. When `false` the
   * plugin performs zero network calls and skips the startup Notice
   * entirely — no silent heartbeat. Default on to match the "check and
   * tell me" expectation most users have for dev-tool plugins.
   */
  checkForUpdates: boolean;
  /**
   * Cached result of the last successful (or failed) update probe. Not
   * surfaced in the settings UI — the Updates section renders this
   * through the backend's read accessor. `undefined` on first install
   * and after a reset, which forces the next `onload` to fetch.
   */
  updateCheck?: UpdateCheckState;
  /**
   * When true, replace remargin fenced blocks in Live Preview and reading
   * mode with rich, read-only widgets. Editing always still happens in
   * the sidebar. Default off; opt-in for the first two releases (T37/T38
   * each gate behind this flag).
   */
  editorWidgets: boolean;
  /**
   * Single global font-scale multiplier for rendered comment markdown,
   * shared by the sidebar and the in-editor widget. Applied as the
   * `--remargin-md-scale` CSS var; the markdown container's base
   * font-size is `calc(var(--remargin-md-scale) * 15.6px)` and every child
   * font rule is `em`-relative, so this one knob scales the whole tree.
   */
  markdownScale: number;
}

/** Bounds for {@link RemarginSettings.markdownScale}. */
export const MARKDOWN_SCALE_MIN = 0.7;
export const MARKDOWN_SCALE_MAX = 2;
export const MARKDOWN_SCALE_STEP = 0.1;
export const MARKDOWN_SCALE_DEFAULT = 1;

/** Clamp to the allowed range and snap float drift to one decimal. */
export function clampMarkdownScale(value: number): number {
  if (!Number.isFinite(value)) return MARKDOWN_SCALE_DEFAULT;
  const clamped = Math.min(MARKDOWN_SCALE_MAX, Math.max(MARKDOWN_SCALE_MIN, value));
  return Math.round(clamped * 100) / 100;
}

export const DEFAULT_SETTINGS: RemarginSettings = {
  remarginPath: "remargin",
  claudePath: "claude",
  terminalCommand: "",
  workingDirectory: "",
  identityMode: "manual",
  configFilePath: "",
  authorName: "user",
  keyFilePath: "",
  sidebarSide: "left",
  sandboxView: "tree",
  inboxView: "tree",
  inboxFilter: "for-me",
  checkForUpdates: true,
  editorWidgets: false,
  markdownScale: MARKDOWN_SCALE_DEFAULT,
};
