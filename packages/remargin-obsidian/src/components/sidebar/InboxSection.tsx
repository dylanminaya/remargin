import { toRegex } from "diacritic-regex";
import { ChevronDown, Clock, FileText, Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { InboxTree } from "@/components/sidebar/InboxTree";
import { inboxFetchToken } from "@/components/sidebar/inboxFetchToken";
import {
  INBOX_FILTER_OPTIONS,
  inboxEmptyMessage,
  inboxFilterLabel,
  inboxFilterQueryOpts,
} from "@/components/sidebar/inboxFilter";
import { identityProbeKey } from "@/components/sidebar/inboxIdentityProbe";
import { deriveLeafState } from "@/components/sidebar/inboxLeafState";
import { KindFilterBar } from "@/components/sidebar/KindFilterBar";
import { MarkdownContent } from "@/components/sidebar/MarkdownContent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import type { ExpandedComment } from "@/generated";
import { useBackend } from "@/hooks/useBackend";
import { useParticipants } from "@/hooks/useParticipants";
import { authorLabel } from "@/lib/authorLabel";
import { collectKinds, matchesKindFilter, pruneKindFilter } from "@/lib/kindFilter";
import type { InboxFilter, ViewMode } from "@/types";

/**
 * Build the diacritic- and case-insensitive pattern shipped to
 * `remargin query --content-regex`. `diacritic-regex` produces character
 * classes like `[CcÇç][AaÀàÁáÂâ...]...` that are compatible with the Rust
 * `regex` crate. The generator leaves non-alpha characters (spaces,
 * punctuation) as literals and leaves consonants without diacritics as
 * lowercase literals — pairing it with the CLI's `--ignore-case` flag
 * promotes those consonants to case-insensitive matches.
 */
const buildSearchPattern = toRegex({ flags: "i" });

interface InboxItem {
  file: string;
  comment: ExpandedComment;
}

interface InboxSectionProps {
  onOpenAtLine?: (filePath: string, line?: number) => void;
  refreshKey?: number;
  /** View mode owned by RemarginSidebar (persisted in plugin settings). */
  viewMode?: ViewMode;
  /** Filter mode owned by RemarginSidebar (persisted in plugin settings). */
  filter?: InboxFilter;
  onFilterChange?: (next: InboxFilter) => void;
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

export function InboxSection({
  onOpenAtLine,
  refreshKey,
  viewMode = "tree",
  filter = "for-me",
  onFilterChange,
}: InboxSectionProps = {}) {
  const backend = useBackend();
  const [items, setItems] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [kindFilter, setKindFilter] = useState<string[]>([]);
  // Resolved identity of the caller. Used by the inbox leaf
  // to decide whether a row is "directed at me" and to detect "acked by
  // me" without a second round-trip. Null while the probe is in flight —
  // leaves render as neutral in that window (see `deriveLeafState`).
  const [me, setMe] = useState<string | null>(null);
  // Separate from `me` because `null` alone cannot tell "probe still in
  // flight" from "probe finished without an identity" — the identity-
  // dependent modes need that distinction to choose between the loading
  // state and the identity-unavailable notice.
  const [identityResolved, setIdentityResolved] = useState(false);
  // The submitted query only advances on explicit user action (Enter key or
  // search-button click). Typing alone does nothing — the old debounce
  // version felt jittery because every keystroke eventually spawned a CLI
  // call after the pause. Manual submit keeps the search intentional.
  const [submittedSearch, setSubmittedSearch] = useState("");

  const isSearching = submittedSearch.trim().length > 0;

  const handleSubmitSearch = useCallback(() => {
    setSubmittedSearch(searchInput.trim());
  }, [searchInput]);

  const handleClearSearch = useCallback(() => {
    setSearchInput("");
    setSubmittedSearch("");
  }, []);

  // Token of the newest fetch issued. Every run tags itself and re-checks
  // the tag before touching state, so a slow response that a newer fetch
  // has already superseded is dropped instead of overwriting it.
  const newestFetch = useRef<string | null>(null);

  const refresh = useCallback(
    async (generation: number) => {
      const token = inboxFetchToken(generation, filter, me, submittedSearch);
      newestFetch.current = token;
      // `from-me` needs a resolved identity to name its `--author`. Skip the
      // fetch entirely rather than querying the whole vault; the render path
      // shows the loading state or the identity-unavailable notice.
      const modeOpts = inboxFilterQueryOpts(filter, me);
      if (!modeOpts) {
        setItems([]);
        setError(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      try {
        // Single refresh path: text filtering composes with every mode via
        // the CLI's own `--content-regex` + `--ignore-case` options so we
        // make exactly one `query` call regardless of search state.
        const opts: Parameters<typeof backend.query>[1] = { ...modeOpts };
        if (isSearching) {
          opts.contentRegex = buildSearchPattern(submittedSearch).source;
          opts.ignoreCase = true;
        }
        const results = await backend.query(".", opts);
        if (newestFetch.current !== token) return;
        const flat: InboxItem[] = [];
        for (const result of results) {
          for (const comment of result.comments ?? []) {
            flat.push({ file: result.path, comment });
          }
        }
        flat.sort((a, b) => (b.comment.ts?.getTime() ?? 0) - (a.comment.ts?.getTime() ?? 0));
        setItems(flat);
        setError(null);
      } catch (err) {
        console.error("InboxSection.refresh failed:", err);
        if (newestFetch.current !== token) return;
        setItems([]);
        setError(errorMessage(err));
      } finally {
        // A superseded run leaves the loading state to the fetch that
        // replaced it, so the list never flashes stale rows in between.
        if (newestFetch.current === token) setLoading(false);
      }
    },
    [backend, filter, me, isSearching, submittedSearch]
  );

  useEffect(() => {
    refresh(refreshKey ?? 0);
  }, [refresh, refreshKey]);

  const probeKey = identityProbeKey(me, refreshKey);

  // Resolve identity once per mount on the healthy path. No retry loop:
  // if the CLI errors we keep `me` as `null` and leaves render as neutral
  // — an acceptable fallback that does not block the inbox from loading.
  // Recovery is user-triggered via the header's Refresh button (which
  // moves `probeKey`), never a timer.
  useEffect(() => {
    if (probeKey === null) return;
    let cancelled = false;
    // A re-probe re-enters the loading state so the identity-unavailable
    // notice does not flash while the new call is in flight.
    setIdentityResolved(false);
    backend
      .identity()
      .then((info) => {
        if (cancelled) return;
        setMe(info.identity ?? null);
        setIdentityResolved(true);
      })
      .catch((err: unknown) => {
        console.error("InboxSection.identity failed:", err);
        if (!cancelled) setIdentityResolved(true);
      });
    return () => {
      cancelled = true;
    };
  }, [backend, probeKey]);

  const filterLabel = useMemo(() => inboxFilterLabel(filter), [filter]);
  const needsIdentity = inboxFilterQueryOpts(filter, me) === null;

  const availableKinds = useMemo(() => collectKinds(items.map((i) => i.comment)), [items]);

  // Drop any selected kinds that are no longer present in the visible
  // set (e.g. after switching from All to Pending).
  useEffect(() => {
    setKindFilter((prev) => pruneKindFilter(prev, availableKinds));
  }, [availableKinds]);

  // Apply the kind filter client-side. The fetch itself is unfiltered
  // so switching chips is instant (no CLI round-trip) and the chip
  // set stays stable — if we filtered on the server and the user
  // selected `question`, we'd never see the other kinds to offer
  // chips for.
  const visibleItems = useMemo(() => {
    if (kindFilter.length === 0) return items;
    return items.filter((i) => matchesKindFilter(i.comment.remargin_kind, kindFilter));
  }, [items, kindFilter]);

  // Hold the loading state while an identity-dependent mode waits on the
  // probe, so the notice below only ever means "the probe finished and
  // produced nothing" rather than "not yet".
  if (loading || (needsIdentity && !identityResolved)) {
    return <div className="px-4 py-3 text-xs text-text-faint">Loading...</div>;
  }

  return (
    <div className="flex flex-col min-w-0">
      <div className="flex flex-col gap-2 px-4 py-2 border-b border-bg-border min-w-0">
        <div className="flex items-center gap-1">
          <div className="relative flex-1">
            <Input
              type="text"
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleSubmitSearch();
                }
              }}
              placeholder="Search comments..."
              aria-label="Search comments"
              className="h-7 text-xs pr-7"
            />
            {searchInput.length > 0 && (
              <button
                type="button"
                aria-label="Clear search"
                onClick={handleClearSearch}
                className="absolute right-1 top-1/2 -translate-y-1/2 p-0.5 text-text-faint hover:text-text-normal"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={handleSubmitSearch}
            aria-label="Search"
            title="Search (Enter)"
            className="h-7 w-7 p-0 shrink-0"
          >
            <Search className="w-3 h-3" />
          </Button>
        </div>
        <div className="flex items-center justify-between gap-2">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="h-7 px-2 text-xs font-medium text-text-normal gap-1.5"
              >
                {filterLabel}
                <ChevronDown className="w-3 h-3 text-text-muted" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="min-w-28">
              {INBOX_FILTER_OPTIONS.map((option) => (
                <DropdownMenuItem
                  key={option.value}
                  onClick={() => onFilterChange?.(option.value)}
                  className="text-xs"
                >
                  {option.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <KindFilterBar
        availableKinds={availableKinds}
        selected={kindFilter}
        onChange={setKindFilter}
      />

      <div className="min-w-0">
        {error ? (
          <div className="px-4 py-3 text-xs text-red-400 whitespace-pre-wrap break-words">
            <div className="font-semibold mb-1">Failed to load inbox</div>
            <div className="font-mono text-[10px]">{error}</div>
          </div>
        ) : needsIdentity ? (
          <div className="px-4 py-3 text-xs text-text-faint">
            <div className="font-semibold mb-1">Identity unavailable</div>
            <div>
              "{filterLabel}" needs your remargin identity, which could not be resolved. Check the
              plugin's identity settings, then click Refresh.
            </div>
          </div>
        ) : visibleItems.length === 0 ? (
          <div className="px-4 py-3 text-xs text-text-faint">
            {isSearching
              ? "No comments match your search."
              : kindFilter.length > 0
                ? "No comments match the selected kinds."
                : inboxEmptyMessage(filter)}
          </div>
        ) : viewMode === "tree" ? (
          <InboxTree items={visibleItems} me={me} onOpenAtLine={onOpenAtLine} />
        ) : (
          <div className="flex flex-col min-w-0">
            {visibleItems.map((item) => (
              <InboxFlatRow
                key={`${item.file}:${item.comment.id}`}
                item={item}
                me={me}
                onOpenAtLine={onOpenAtLine}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface InboxFlatRowProps {
  item: InboxItem;
  me: string | null;
  onOpenAtLine?: (filePath: string, line?: number) => void;
}

/**
 * Single row in the inbox's flat (non-tree) view. Extracted as its own
 * component so it can call `useParticipants` at the row level — hooks
 * cannot run inside a `.map` callback.
 *
 * Renders one of three visuals derived from `deriveLeafState`:
 * `me-directed-unacked` (purple accent), `acked-by-me` (dimmed), or
 * `neutral` (default styling). Ack/Unack is intentionally NOT offered
 * here — acking from an inbox card would ack without context. The user
 * clicks the row to open the comment in its file, where the comment
 * card exposes the canonical Ack affordance.
 */
function InboxFlatRow({ item, me, onOpenAtLine }: InboxFlatRowProps) {
  const { resolveDisplayName } = useParticipants();
  const { label: authorDisplay, title: authorTitle } = authorLabel(
    item.comment.author,
    resolveDisplayName
  );
  const { visual } = deriveLeafState(item.comment, me);
  const visualCls =
    visual === "me-directed-unacked"
      ? "border-l-2 border-l-purple-500 bg-purple-500/5 hover:bg-purple-500/10"
      : visual === "acked-by-me"
        ? "opacity-60"
        : "hover:bg-bg-hover";
  return (
    <div
      className={`flex flex-col gap-1 px-4 py-2 border-b border-bg-border cursor-pointer min-w-0 overflow-hidden ${visualCls}`}
      onClick={() => onOpenAtLine?.(item.file, item.comment.line)}
    >
      <div className="flex items-center justify-between gap-2 min-w-0">
        <div className="flex items-center gap-1.5 min-w-0 flex-1">
          <Badge
            className={`px-1 py-0 text-[9px] font-semibold shrink-0 ${
              item.comment.author_type === "agent"
                ? "bg-purple-400 text-white"
                : "bg-blue-400 text-white"
            }`}
          >
            {item.comment.author_type === "agent" ? "AI" : "H"}
          </Badge>
          {item.comment.id && (
            <Badge className="px-1 py-0 text-[9px] font-mono font-semibold bg-slate-500 text-white shrink-0">
              {item.comment.id}
            </Badge>
          )}
          {item.comment.line > 0 && (
            <span className="text-[9px] text-text-faint font-mono shrink-0">
              L{item.comment.line}
            </span>
          )}
          <span
            className="text-xs font-medium text-text-normal truncate min-w-0"
            title={authorTitle}
          >
            {authorDisplay}
          </span>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <Clock className="w-3 h-3 text-text-faint" />
          <span className="text-[10px] text-text-faint whitespace-nowrap">
            {formatRelativeTime(item.comment.ts)}
          </span>
        </div>
      </div>
      <div className="line-clamp-2 overflow-hidden min-w-0">
        <MarkdownContent
          content={item.comment.content ?? ""}
          sourcePath={item.file}
          className="min-w-0"
        />
      </div>
      <div className="flex items-center justify-between gap-2 min-w-0">
        <div className="flex items-center gap-1 min-w-0 flex-1">
          <FileText className="w-3 h-3 text-text-faint shrink-0" />
          <span className="font-mono text-[10px] text-text-faint truncate min-w-0">
            {item.file}
          </span>
        </div>
      </div>
    </div>
  );
}

function formatRelativeTime(ts?: string | Date): string {
  if (!ts) return "";
  try {
    const diff = Date.now() - new Date(ts).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "now";
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    return `${days}d`;
  } catch {
    return "";
  }
}
