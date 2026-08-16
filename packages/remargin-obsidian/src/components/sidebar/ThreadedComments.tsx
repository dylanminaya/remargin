import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { CommentCard } from "@/components/sidebar/CommentCard";
import { KindFilterBar } from "@/components/sidebar/KindFilterBar";
import { readToken } from "@/components/sidebar/readToken";
import { findRadixScrollViewport } from "@/components/sidebar/scrollViewport";
import type { Comment } from "@/generated";
import { useBackend } from "@/hooks/useBackend";
import { collectKinds, matchesKindFilter, pruneKindFilter } from "@/lib/kindFilter";
import { buildThreadTree, type ThreadNode } from "@/lib/threadTree";
import { parseVerifyFailure, type VerifyFailure } from "@/lib/verifyFailure";

interface ThreadedCommentsProps {
  file: string;
  onReply?: (commentId: string) => void;
  onGoToLine?: (line: number) => void;
  onMutation?: () => void;
  /**
   * Monotonic counter bumped by the shell when any sidebar section
   * should refetch. We observe it as a prop (rather than being keyed
   * off it) so the component refetches in place, preserving the scroll
   * offset of the outer sidebar viewport.
   */
  refreshKey?: number;
  /**
   * ID of the comment the user is replying to, if any. When set, the
   * `replyEditor` node is rendered as a peer row immediately after the
   * matching comment's card (same visual depth as a reply) instead of at
   * the top of the thread, so the composer stays next to the comment the
   * user is actually replying to.
   */
  replyTarget?: string | null;
  /**
   * The inline reply composer to render below the targeted comment. Owned
   * by the sidebar (which also owns `replyTarget`), passed down so the
   * thread can slot it in at the right place.
   */
  replyEditor?: React.ReactNode;
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

export function ThreadedComments({
  file,
  onReply,
  onGoToLine,
  onMutation,
  refreshKey,
  replyTarget,
  replyEditor,
}: ThreadedCommentsProps) {
  const backend = useBackend();
  const [comments, setComments] = useState<Comment[]>([]);
  const [kindFilter, setKindFilter] = useState<string[]>([]);
  // `loading` is true only until the very first fetch for a given file
  // resolves. Subsequent refetches (from refreshKey bumps, reactions,
  // acks, or reply submits) do NOT flip this back to true — that would
  // collapse the rendered list to a "Loading..." placeholder and the
  // outer sidebar viewport would snap to scrollTop=0.
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [me, setMe] = useState<string | null>(null);
  // Scroll offset to reinstate once a refetched list commits, so a reply
  // doesn't jump the list to the top. A fresh object per refetch, so the
  // layout effect below still fires when the offset repeats.
  const [scrollRestore, setScrollRestore] = useState<{ top: number } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollViewportRef = useRef<HTMLElement | null>(null);

  const findScrollViewport = useCallback((): HTMLElement | null => {
    if (scrollViewportRef.current && document.contains(scrollViewportRef.current)) {
      return scrollViewportRef.current;
    }
    const found = findRadixScrollViewport(rootRef.current);
    if (found) scrollViewportRef.current = found;
    return found;
  }, []);

  // Token of the newest fetch issued. Every run tags itself and re-checks
  // the tag before touching state, so a slow response that a newer fetch
  // has already superseded is dropped instead of overwriting it.
  const newestFetch = useRef<string | null>(null);
  // File the rendered list belongs to. A mismatch means the user switched
  // files and what is on screen no longer describes `file`.
  const renderedFile = useRef<string | null>(null);

  const refresh = useCallback(
    async (generation: number) => {
      const token = readToken(generation, file);
      newestFetch.current = token;
      if (renderedFile.current !== file) {
        renderedFile.current = file;
        // Switching files is the one case where we do want the
        // placeholder back: the previous file's list means nothing here.
        setLoading(true);
        setComments([]);
      }
      // Snapshot the current scroll offset so it can be reinstated once
      // the new comment list commits. Only matters for in-place
      // refetches — on first mount there's nothing to preserve.
      const snapshot = findScrollViewport()?.scrollTop ?? null;
      try {
        const result = await backend.comments(file);
        if (newestFetch.current !== token) return;
        setComments(result);
        setError(null);
      } catch (err) {
        console.error("ThreadedComments.refresh failed:", err);
        if (newestFetch.current !== token) return;
        setComments([]);
        setError(errorMessage(err));
      } finally {
        // A superseded run leaves the commit to the fetch that replaced
        // it, so the list never flashes stale rows in between.
        if (newestFetch.current === token) {
          setLoading(false);
          if (snapshot !== null) setScrollRestore({ top: snapshot });
        }
      }
    },
    [backend, file, findScrollViewport]
  );

  useEffect(() => {
    refresh(refreshKey ?? 0);
  }, [refresh, refreshKey]);

  // Reinstate the viewport's scrollTop synchronously after the refetched
  // list commits, so the user doesn't see the scroll jump. Pairs with the
  // snapshot taken inside `refresh()`.
  useLayoutEffect(() => {
    if (scrollRestore === null) return;
    const viewport = findScrollViewport();
    if (viewport) {
      viewport.scrollTop = scrollRestore.top;
    }
  }, [scrollRestore, findScrollViewport]);

  // Resolve the current identity once per mount so reaction pills can
  // distinguish "mine" from others' without threading it in from the shell.
  useEffect(() => {
    let cancelled = false;
    backend
      .identity()
      .then((info) => {
        if (!cancelled) setMe(info.identity ?? null);
      })
      .catch((err) => {
        console.error("ThreadedComments.identity failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const availableKinds = useMemo(() => collectKinds(comments), [comments]);

  // Drop any selected kinds that are no longer present in the visible set.
  useEffect(() => {
    setKindFilter((prev) => pruneKindFilter(prev, availableKinds));
  }, [availableKinds]);

  // Apply the kind filter client-side. Filtering at the comment level
  // (not thread level) matches the CLI semantics and lets a reply that
  // carries the filtered kind stay visible even when its parent does
  // not. Orphans naturally float up to root via `buildThreadTree`
  // because it treats a missing `reply_to` parent as "no parent".
  const visibleComments = useMemo(() => {
    if (kindFilter.length === 0) return comments;
    return comments.filter((c) => matchesKindFilter(c.remargin_kind ?? [], kindFilter));
  }, [comments, kindFilter]);

  const threads = useMemo(() => buildThreadTree(visibleComments), [visibleComments]);

  const handleAck = useCallback(
    async (id: string, remove: boolean) => {
      try {
        await backend.ack(file, [id], remove);
        // Stage the file in the user's sandbox so the interaction is
        // visible in the next Submit-to-Claude cycle.
        try {
          await backend.sandboxAdd([file]);
        } catch {
          // Best-effort: ack succeeded, don't fail the whole operation.
        }
        await refresh(refreshKey ?? 0);
        onMutation?.();
      } catch (err) {
        console.error("ThreadedComments.ack failed:", err);
        setError(errorMessage(err));
      }
    },
    [backend, file, refresh, refreshKey, onMutation]
  );

  const handleReact = useCallback(
    async (id: string, emoji: string, remove: boolean) => {
      try {
        await backend.react(file, id, emoji, remove);
        await refresh(refreshKey ?? 0);
        onMutation?.();
      } catch (err) {
        console.error("ThreadedComments.react failed:", err);
        setError(errorMessage(err));
      }
    },
    [backend, file, refresh, refreshKey, onMutation]
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await backend.deleteComments(file, [id]);
        await refresh(refreshKey ?? 0);
        onMutation?.();
      } catch (err) {
        console.error("ThreadedComments.delete failed:", err);
        setError(errorMessage(err));
      }
    },
    [backend, file, refresh, refreshKey, onMutation]
  );

  if (loading) {
    return (
      <div ref={rootRef} className="px-4 py-3 text-xs text-text-faint">
        Loading...
      </div>
    );
  }

  if (error) {
    return (
      <div ref={rootRef} className="px-4 py-3 text-xs text-red-400 whitespace-pre-wrap break-words">
        <ErrorPanel raw={error} />
      </div>
    );
  }

  if (threads.length === 0) {
    const filtered = comments.length > 0 && kindFilter.length > 0;
    return (
      <div ref={rootRef}>
        {filtered && (
          <KindFilterBar
            availableKinds={availableKinds}
            selected={kindFilter}
            onChange={setKindFilter}
          />
        )}
        <div className="px-4 py-3 text-xs text-text-faint">
          {filtered ? "No comments match the selected kinds." : "No comments in this file."}
        </div>
      </div>
    );
  }

  return (
    <div ref={rootRef}>
      <KindFilterBar
        availableKinds={availableKinds}
        selected={kindFilter}
        onChange={setKindFilter}
      />
      <div className="flex flex-col">
        {threads.map((node) => (
          <CommentThread
            key={node.comment.id}
            node={node}
            file={file}
            depth={0}
            me={me}
            onAck={handleAck}
            onDelete={handleDelete}
            onReply={onReply}
            onReact={handleReact}
            onGoToLine={onGoToLine}
            replyTarget={replyTarget ?? null}
            replyEditor={replyEditor}
          />
        ))}
      </div>
    </div>
  );
}

/**
 * Render the comment-pane error state. When the raw stderr blob carries
 * the structured `verify_failed` shape, surface a plain-English headline
 * + actionable hint, with the per-failure breakdown tucked inside a
 * disclosure. Falls back to the raw text otherwise.
 */
function ErrorPanel({ raw }: { raw: string }) {
  const parsed: VerifyFailure | null = parseVerifyFailure(raw);
  if (!parsed) {
    return (
      <>
        <div className="font-semibold mb-1">Failed to load comments</div>
        <div className="font-mono text-[10px]">{raw}</div>
      </>
    );
  }
  return (
    <>
      <div className="font-semibold mb-1">{parsed.headline}</div>
      <div className="mb-2">{parsed.hint}</div>
      <details>
        <summary className="cursor-pointer">Show full details</summary>
        <ul className="font-mono text-[10px] mt-1">
          {parsed.failures.map((row) => (
            <li key={row.id}>
              {row.id}: checksum={row.checksum_ok ? "ok" : "FAIL"} signature={row.signature}
            </li>
          ))}
        </ul>
      </details>
    </>
  );
}

interface CommentThreadProps {
  node: ThreadNode;
  file: string;
  depth: number;
  me: string | null;
  onAck: (id: string, remove: boolean) => void;
  onDelete: (id: string) => void;
  onReply?: (id: string) => void;
  onReact: (id: string, emoji: string, remove: boolean) => void;
  onGoToLine?: (line: number) => void;
  /**
   * ID of the comment whose card should have the inline reply editor
   * rendered directly beneath it (nested one level deeper, matching the
   * depth a real reply would render at). Compared against this node's id
   * during traversal — only one match fires.
   */
  replyTarget: string | null;
  replyEditor?: React.ReactNode;
}

function CommentThread({
  node,
  file,
  depth,
  me,
  onAck,
  onDelete,
  onReply,
  onReact,
  onGoToLine,
  replyTarget,
  replyEditor,
}: CommentThreadProps) {
  const isReplyHere = replyTarget === node.comment.id && !!replyEditor;
  return (
    <div>
      <CommentCard
        comment={node.comment}
        file={file}
        depth={depth}
        isOnline={false}
        me={me}
        onAck={onAck}
        onDelete={onDelete}
        onReply={onReply}
        onReact={onReact}
        onGoToLine={onGoToLine}
      />
      {isReplyHere && <InlineReplySlot depth={depth + 1}>{replyEditor}</InlineReplySlot>}
      {node.replies.map((reply) => (
        <CommentThread
          key={reply.comment.id}
          node={reply}
          file={file}
          depth={depth + 1}
          me={me}
          onAck={onAck}
          onDelete={onDelete}
          onReply={onReply}
          onReact={onReact}
          onGoToLine={onGoToLine}
          replyTarget={replyTarget}
          replyEditor={replyEditor}
        />
      ))}
    </div>
  );
}

/**
 * Wrapper that scrolls the inline reply editor into view on mount so the
 * user does not lose it on a long thread. Depth controls the left inset
 * so the composer visually nests under the comment being replied to.
 */
function InlineReplySlot({ depth, children }: { depth: number; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, []);
  // Match CommentCard's depth-based left padding (10px base + 16px per
  // level) so the composer aligns with comment cards at the same depth.
  const style = { paddingLeft: `${10 + depth * 16}px` };
  return (
    <div ref={ref} style={style}>
      {children}
    </div>
  );
}
