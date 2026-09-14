import { useCallback, useEffect, useRef, useState } from "react";

export type SessionStatus = "idle" | "running" | "cancelling" | "error" | "archived";
export type SessionCopyStrategy = "git-worktree" | "full" | "shallow" | "legacy";

export interface SessionRecord {
  sessionId: string;
  name: string;
  description: string | null;
  projectRoot: string;
  currentRunId: string | null;
  status: SessionStatus;
  wsPort: number | null;
  pid: number | null;
  createdAt: number;
  updatedAt: number;
  metadata: {
    originalPath: string;
    copyStrategy: SessionCopyStrategy;
    diskUsageBytes?: number;
  };
}

export interface UseSessionsResult {
  sessions: SessionRecord[];
  controlStatus: "connecting" | "open" | "closed";
  /** Last error from a server-rejected create/start/etc, surfaced for toasts. */
  lastError: { kind: string; message: string } | null;
  refresh: () => void;
  createSession: (input: {
    name: string;
    description?: string;
    sourcePath: string;
    strategy: SessionCopyStrategy;
  }) => void;
  startSession: (sessionId: string) => Promise<{ wsPort: number; runId: string } | null>;
  stopSession: (sessionId: string) => void;
  archiveSession: (sessionId: string) => void;
  deleteSession: (sessionId: string) => void;
}

/**
 * Subscribes to the supervisor's control WS at `controlUrl` (default
 * ws://<host>:VITE_WS_PORT). Maintains the live list of sessions and
 * exposes commands. Each command is fire-and-forget except startSession,
 * which awaits the supervisor's `sessions:start:response` so the caller
 * can immediately open a per-session WS to the returned port.
 */
export function useSessions(controlUrl: string): UseSessionsResult {
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [controlStatus, setControlStatus] = useState<UseSessionsResult["controlStatus"]>("connecting");
  const [lastError, setLastError] = useState<UseSessionsResult["lastError"]>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const startWaitersRef = useRef(
    new Map<string, (v: { wsPort: number; runId: string } | null) => void>()
  );

  useEffect(() => {
    let stopped = false;
    let retryTimer: number | null = null;

    const connect = () => {
      setControlStatus("connecting");
      const ws = new WebSocket(controlUrl);
      wsRef.current = ws;

      // All handlers identity-check `wsRef.current === ws` before touching
      // shared state. Without this, in React 18 StrictMode the *previous*
      // socket's deferred onclose can fire AFTER a new socket is in place
      // and silently null the ref — leaving send() permanently broken.
      ws.onopen = () => {
        if (wsRef.current !== ws) return;
        setControlStatus("open");
        // Phase 5: register as a session-less dashboard so the supervisor
        // knows we only want `sessions:*` broadcasts. Pipeline events are
        // scoped per-session and reach a different connection (the one
        // opened by usePipeline with a sessionId).
        try {
          ws.send(
            JSON.stringify({
              type: "hello",
              payload: { role: "dashboard" },
            })
          );
        } catch {
          /* will retry on reconnect */
        }
      };
      ws.onclose = () => {
        if (wsRef.current === ws) {
          setControlStatus("closed");
          wsRef.current = null;
          if (!stopped) {
            retryTimer = window.setTimeout(connect, 1500);
          }
        }
      };
      ws.onerror = () => {
        // onclose will fire after this; reconnect handled there.
      };
      ws.onmessage = (ev) => {
        let msg: { type: string; payload?: any };
        try {
          msg = JSON.parse(ev.data);
        } catch {
          return;
        }
        const { type, payload } = msg;
        switch (type) {
          case "sessions:snapshot":
          case "sessions:list:response": {
            setSessions(payload?.sessions ?? []);
            break;
          }
          case "sessions:created": {
            setSessions((prev) => upsert(prev, payload.session));
            break;
          }
          case "sessions:archived": {
            setSessions((prev) =>
              prev.map((s) =>
                s.sessionId === payload.sessionId ? { ...s, status: "archived" as const } : s
              )
            );
            break;
          }
          case "sessions:deleted": {
            setSessions((prev) => prev.filter((s) => s.sessionId !== payload.sessionId));
            break;
          }
          case "sessions:started": {
            setSessions((prev) =>
              prev.map((s) =>
                s.sessionId === payload.sessionId
                  ? {
                      ...s,
                      status: "running" as const,
                      wsPort: payload.wsPort,
                      pid: payload.pid,
                      currentRunId: payload.runId,
                    }
                  : s
              )
            );
            const w = startWaitersRef.current.get(payload.sessionId);
            if (w) {
              w({ wsPort: payload.wsPort, runId: payload.runId });
              startWaitersRef.current.delete(payload.sessionId);
            }
            break;
          }
          case "sessions:exited": {
            setSessions((prev) =>
              prev.map((s) =>
                s.sessionId === payload.sessionId
                  ? { ...s, status: "idle" as const, wsPort: null, pid: null, currentRunId: null }
                  : s
              )
            );
            break;
          }
          case "sessions:start:error": {
            setLastError({ kind: "start", message: payload?.error ?? "unknown" });
            const w = startWaitersRef.current.get(payload.sessionId);
            if (w) {
              w(null);
              startWaitersRef.current.delete(payload.sessionId);
            }
            break;
          }
          case "sessions:create:error":
          case "sessions:archive:error":
          case "sessions:delete:error": {
            setLastError({
              kind: type.replace("sessions:", "").replace(":error", ""),
              message: payload?.error ?? "unknown",
            });
            break;
          }
        }
      };
    };

    connect();

    return () => {
      stopped = true;
      if (retryTimer) window.clearTimeout(retryTimer);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [controlUrl]);

  const send = useCallback((type: string, payload: unknown) => {
    const ws = wsRef.current;
    if (!ws) {
      // eslint-disable-next-line no-console
      console.warn("[useSessions.send] no ws", type);
      return;
    }
    if (ws.readyState !== WebSocket.OPEN) {
      // eslint-disable-next-line no-console
      console.warn(
        "[useSessions.send] ws not open, dropping",
        type,
        "readyState=",
        ws.readyState
      );
      return;
    }
    ws.send(JSON.stringify({ type, payload }));
    // eslint-disable-next-line no-console
    console.log("[useSessions.send]", type);
  }, []);

  return {
    sessions,
    controlStatus,
    lastError,
    refresh: useCallback(() => send("sessions:list", {}), [send]),
    createSession: useCallback((input) => send("sessions:create", input), [send]),
    startSession: useCallback(
      (sessionId) =>
        new Promise<{ wsPort: number; runId: string } | null>((resolve) => {
          startWaitersRef.current.set(sessionId, resolve);
          send("sessions:start", { sessionId });
          // Safety timeout — if the supervisor never replies we don't hang
          // the caller forever. 8s is generous; child spawn is sub-second.
          window.setTimeout(() => {
            const w = startWaitersRef.current.get(sessionId);
            if (w) {
              w(null);
              startWaitersRef.current.delete(sessionId);
            }
          }, 8000);
        }),
      [send]
    ),
    stopSession: useCallback((sessionId) => send("sessions:stop", { sessionId }), [send]),
    archiveSession: useCallback(
      (sessionId) => send("sessions:archive", { sessionId }),
      [send]
    ),
    deleteSession: useCallback(
      (sessionId) => send("sessions:delete", { sessionId }),
      [send]
    ),
  };
}

function upsert(list: SessionRecord[], s: SessionRecord): SessionRecord[] {
  const idx = list.findIndex((x) => x.sessionId === s.sessionId);
  if (idx === -1) return [s, ...list];
  const next = list.slice();
  next[idx] = s;
  return next;
}
