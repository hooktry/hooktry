import {
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import {
  authSession,
  claimHook,
  createHook,
  githubSignInUrl,
  websocketUrl,
} from "./api";
import {
  formatBytes,
  formatTimestamp,
  interactionMatches,
  mergeInteraction,
  prettyBody,
  viewCapabilityFromPath,
} from "./model";
import {
  clearOwnerProvision,
  loadOwnerProvision,
  saveOwnerProvision,
} from "./session";
import type {
  ExposureSummary,
  HookProvision,
  Interaction,
  StreamFrame,
} from "./types";

type ConnectionState = "idle" | "connecting" | "live" | "disconnected" | "error";
type InspectorTab = "body" | "headers" | "metadata";

export function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const [provision, setProvision] = useState<HookProvision | null>(() =>
    loadOwnerProvision(window.location.pathname),
  );
  const [summary, setSummary] = useState<ExposureSummary | null>(
    provision,
  );
  const [interactions, setInteractions] = useState<Interaction[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("idle");
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<InspectorTab>("body");
  const [creating, setCreating] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const viewCapability = viewCapabilityFromPath(pathname);
  const viewing = Boolean(viewCapability || provision);
  const owner = Boolean(provision);
  const activeSummary = summary ?? provision;

  const viewWebSocketUrl = useMemo(() => {
    if (provision) return provision.view_websocket_url;
    if (!viewCapability) return null;
    return websocketUrl(window.location.href);
  }, [provision, viewCapability]);

  useEffect(() => {
    const onPopState = () => {
      const nextPath = window.location.pathname;
      setPathname(nextPath);
      const stored = loadOwnerProvision(nextPath);
      setProvision(stored);
      setSummary(stored);
      setInteractions([]);
      setSelectedId(null);
      setSearch("");
      setError(null);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  useEffect(() => {
    if (!viewWebSocketUrl) {
      setConnection("idle");
      return;
    }

    let stopped = false;
    let socket: WebSocket | null = null;
    let retry: ReturnType<typeof setTimeout> | null = null;

    const connect = () => {
      if (stopped) return;

      setConnection("connecting");
      socket = new WebSocket(viewWebSocketUrl);

      socket.addEventListener("open", () => {
        if (!stopped) setConnection("live");
      });

      socket.addEventListener("message", (event) => {
        try {
          const frame = JSON.parse(String(event.data)) as StreamFrame;
          if (frame.type === "ready") {
            setSummary(frame.exposure);
            return;
          }

          if (frame.type === "interaction") {
            setInteractions((current) =>
              mergeInteraction(current, frame.interaction),
            );
            setSelectedId((current) => current ?? frame.interaction.interaction_id);
            setSummary((current) => {
              if (!current || frame.interaction.sequence <= current.request_count) {
                return current;
              }
              return {
                ...current,
                request_count: frame.interaction.sequence,
                retained_bytes:
                  current.retained_bytes + frame.interaction.body_bytes,
              };
            });
            return;
          }

          if (frame.type === "resync_required") {
            socket?.close(1012, "resync_required");
          }
        } catch {
          setError("Received an unreadable stream frame.");
        }
      });

      socket.addEventListener("close", () => {
        if (stopped) return;
        setConnection("disconnected");
        retry = setTimeout(connect, 1200);
      });

      socket.addEventListener("error", () => {
        if (!stopped) setConnection("error");
      });
    };

    connect();

    return () => {
      stopped = true;
      if (retry) clearTimeout(retry);
      socket?.close(1000, "viewer_changed");
    };
  }, [viewWebSocketUrl]);

  useEffect(() => {
    if (!provision || provision.claimed) return;

    const url = new URL(window.location.href);
    if (url.searchParams.get("claim") !== "1") return;

    url.searchParams.delete("claim");
    window.history.replaceState(
      {},
      "",
      `${url.pathname}${url.search}${url.hash}`,
    );
    void completeClaim(provision);
  }, [provision]);

  const filtered = useMemo(
    () => interactions.filter((interaction) => interactionMatches(interaction, search)),
    [interactions, search],
  );
  const selected =
    interactions.find((interaction) => interaction.interaction_id === selectedId) ??
    filtered[0] ??
    null;

  async function handleCreate() {
    setCreating(true);
    setError(null);

    try {
      const created = await createHook();
      saveOwnerProvision(created);
      const path = new URL(created.view_url, window.location.href).pathname;
      window.history.pushState({}, "", path);
      setPathname(path);
      setProvision(created);
      setSummary(created);
      setInteractions([]);
      setSelectedId(null);
      setSearch("");
      setTab("body");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Unable to create Hook.");
    } finally {
      setCreating(false);
    }
  }

  async function handleClaim() {
    if (!provision || provision.claimed || claiming) return;

    setClaiming(true);
    setError(null);

    try {
      const session = await authSession();
      if (!session.authenticated) {
        const returnTo = `${pathname}?claim=1`;
        window.location.assign(githubSignInUrl(returnTo));
        return;
      }

      await claimCurrent(provision);
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : "Unable to start claim flow.",
      );
    } finally {
      setClaiming(false);
    }
  }

  async function completeClaim(current: HookProvision) {
    if (claiming) return;

    setClaiming(true);
    setError(null);
    try {
      const session = await authSession();
      if (!session.authenticated) {
        throw new Error("GitHub sign-in did not create an Hooktry session.");
      }
      await claimCurrent(current);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "Unable to claim Hook.",
      );
    } finally {
      setClaiming(false);
    }
  }

  async function claimCurrent(current: HookProvision) {
    const claimed = await claimHook(current.claim_url);
    const updated: HookProvision = {
      ...current,
      ...claimed,
    };
    saveOwnerProvision(updated);
    setProvision(updated);
    setSummary(claimed);
  }

  function handleNewHook() {
    clearOwnerProvision(provision);
    window.history.pushState({}, "", "/");
    setPathname("/");
    setProvision(null);
    setSummary(null);
    setInteractions([]);
    setSelectedId(null);
    setConnection("idle");
    setSearch("");
    setError(null);
  }

  async function copy(value: string, key: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(key);
      setTimeout(() => setCopied((current) => (current === key ? null : current)), 1200);
    } catch {
      setError("Clipboard access was denied by the browser.");
    }
  }

  return (
    <div className="app-shell">
      <Sidebar viewing={viewing} onNewHook={handleNewHook} />
      <main className="main">
        <Topbar connection={connection} viewing={viewing} owner={owner} />

        {!viewing ? (
          <Landing creating={creating} error={error} onCreate={handleCreate} />
        ) : (
          <section className="workspace">
            <HookHeader
              provision={provision}
              summary={activeSummary}
              copied={copied}
              claiming={claiming}
              onClaim={handleClaim}
              onCopy={copy}
              onNewHook={handleNewHook}
            />

            {error ? <div className="error-banner">{error}</div> : null}

            <div className="workspace-grid">
              <InteractionList
                interactions={filtered}
                total={interactions.length}
                selectedId={selected?.interaction_id ?? null}
                search={search}
                onSearch={setSearch}
                onSelect={setSelectedId}
              />
              <Inspector
                interaction={selected}
                tab={tab}
                onTab={setTab}
                copied={copied}
                onCopy={copy}
              />
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

function Sidebar({
  viewing,
  onNewHook,
}: {
  viewing: boolean;
  onNewHook: () => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">O</span>
        <span>Hooktry</span>
      </div>

      <nav className="nav">
        <button className="nav-item active" type="button">
          <span>Hooks</span>
          <kbd>H</kbd>
        </button>
        <div className="nav-section">Evidence</div>
        <button className="nav-item muted" type="button" disabled>
          Recordings
        </button>
        <button className="nav-item muted" type="button" disabled>
          Replays
        </button>
        <button className="nav-item muted" type="button" disabled>
          Contracts
        </button>
        <button className="nav-item muted" type="button" disabled>
          Scenarios
        </button>
      </nav>

      <div className="sidebar-footer">
        {viewing ? (
          <button className="button secondary full" type="button" onClick={onNewHook}>
            + New Hook
          </button>
        ) : null}
        <div className="runtime-label">
          <span className="dot" />
          shared web surface
        </div>
      </div>
    </aside>
  );
}

function Topbar({
  connection,
  viewing,
  owner,
}: {
  connection: ConnectionState;
  viewing: boolean;
  owner: boolean;
}) {
  return (
    <header className="topbar">
      <div className="breadcrumb">
        <span>Workspace</span>
        <span className="sep">/</span>
        <strong>Hooks</strong>
      </div>
      <div className="topbar-actions">
        {viewing ? (
          <>
            <span className={`connection ${connection}`}>
              <span className="dot" />
              {connectionLabel(connection)}
            </span>
            <span className="badge">{owner ? "owner session" : "read-only capability"}</span>
          </>
        ) : (
          <span className="badge">no account required</span>
        )}
      </div>
    </header>
  );
}

function Landing({
  creating,
  error,
  onCreate,
}: {
  creating: boolean;
  error: string | null;
  onCreate: () => void;
}) {
  return (
    <section className="landing">
      <div className="landing-card">
        <div className="eyebrow">EPHEMERAL HOOK</div>
        <h1>Receive a webhook. Understand it immediately.</h1>
        <p>
          Create a public Hook without an account. Requests appear live as structured
          interactions you can inspect and hand to an agent.
        </p>

        <div className="policy-grid">
          <Policy value="5 days" label="ephemeral lifetime" />
          <Policy value="100" label="requests per Hook" />
          <Policy value="5 MiB" label="per request" />
          <Policy value="50 MiB" label="retained bodies" />
        </div>

        <button className="button primary create" type="button" onClick={onCreate} disabled={creating}>
          {creating ? "Creating…" : "Create ephemeral Hook"}
        </button>

        {error ? <div className="error-inline">{error}</div> : null}

        <div className="landing-footnote">
          Hook, view, and claim use separate bearer capabilities. Anonymous data expires
          unless claimed into a workspace.
        </div>
      </div>
    </section>
  );
}

function Policy({ value, label }: { value: string; label: string }) {
  return (
    <div className="policy">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

function HookHeader({
  provision,
  summary,
  copied,
  claiming,
  onClaim,
  onCopy,
  onNewHook,
}: {
  provision: HookProvision | null;
  summary: ExposureSummary | null;
  copied: string | null;
  claiming: boolean;
  onClaim: () => void;
  onCopy: (value: string, key: string) => void;
  onNewHook: () => void;
}) {
  return (
    <div className="hook-header">
      <div className="hook-title-row">
        <div>
          <div className="eyebrow">
            {summary?.claimed ? "PERSISTENT HOOK" : "EPHEMERAL HOOK"}
          </div>
          <h1>{summary ? shortId(summary.exposure_id) : "Loading viewer…"}</h1>
        </div>
        <div className="header-actions">
          {provision && !summary?.claimed ? (
            <button
              className="button secondary"
              type="button"
              onClick={onClaim}
              disabled={claiming}
            >
              {claiming ? "Claiming…" : "Claim · sign in with GitHub"}
            </button>
          ) : null}
          {summary?.claimed ? (
            <span className="badge claimed-badge">claimed · persistent</span>
          ) : null}
          <button className="button ghost" type="button" onClick={onNewHook}>
            New Hook
          </button>
        </div>
      </div>

      {provision ? (
        <div className="url-box">
          <div className="url-label">Public ingress</div>
          <code>{provision.hook_url}</code>
          <button
            className="copy-button"
            type="button"
            onClick={() => onCopy(provision.hook_url, "hook")}
          >
            {copied === "hook" ? "Copied" : "Copy"}
          </button>
        </div>
      ) : (
        <div className="readonly-note">
          This URL carries read authority only. The hook and claim capabilities are not
          derivable from it.
        </div>
      )}

      <div className="metrics">
        <Metric
          label="Requests"
          value={summary ? `${summary.request_count} / ${summary.request_limit}` : "—"}
        />
        <Metric
          label="Retained"
          value={summary ? `${formatBytes(summary.retained_bytes)} / ${formatBytes(summary.max_retained_bytes)}` : "—"}
        />
        <Metric
          label="Max body"
          value={summary ? formatBytes(summary.max_body_bytes) : "—"}
        />
        <Metric
          label="Expires"
          value={summary?.claimed ? "persistent" : expiryLabel(summary?.expires_at_unix_seconds)}
        />
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function InteractionList({
  interactions,
  total,
  selectedId,
  search,
  onSearch,
  onSelect,
}: {
  interactions: Interaction[];
  total: number;
  selectedId: string | null;
  search: string;
  onSearch: (value: string) => void;
  onSelect: (id: string) => void;
}) {
  return (
    <section className="panel interaction-panel">
      <div className="panel-header">
        <div>
          <strong>Interactions</strong>
          <span className="count">{total}</span>
        </div>
        <span className="live-hint">live stream</span>
      </div>
      <div className="search-row">
        <input
          aria-label="Filter interactions"
          placeholder="Filter method, path, header, body…"
          value={search}
          onChange={(event) => onSearch(event.target.value)}
        />
      </div>
      <div className="interaction-list">
        {interactions.length === 0 ? (
          <div className="empty-list">
            <strong>{search ? "No matches" : "Waiting for a request"}</strong>
            <span>
              {search
                ? "Try another filter."
                : "Send an HTTP request to the public ingress URL."}
            </span>
          </div>
        ) : (
          interactions.map((interaction) => (
            <button
              key={interaction.interaction_id}
              type="button"
              className={`interaction-row ${selectedId === interaction.interaction_id ? "selected" : ""}`}
              onClick={() => onSelect(interaction.interaction_id)}
            >
              <div className="interaction-main">
                <span className={`method method-${interaction.method.toLowerCase()}`}>
                  {interaction.method}
                </span>
                <span className="path">
                  {interaction.path}
                  {interaction.query ? <span className="query">?{interaction.query}</span> : null}
                </span>
              </div>
              <div className="interaction-meta">
                <span>#{interaction.sequence}</span>
                <span>{formatBytes(interaction.body_bytes)}</span>
                <span>{formatTimestamp(interaction.received_at_unix_ms)}</span>
              </div>
            </button>
          ))
        )}
      </div>
    </section>
  );
}

function Inspector({
  interaction,
  tab,
  onTab,
  copied,
  onCopy,
}: {
  interaction: Interaction | null;
  tab: InspectorTab;
  onTab: (tab: InspectorTab) => void;
  copied: string | null;
  onCopy: (value: string, key: string) => void;
}) {
  if (!interaction) {
    return (
      <section className="panel inspector-panel empty-inspector">
        <div>
          <div className="eyebrow">INSPECTOR</div>
          <h2>No Interaction selected</h2>
          <p>The newest request will be selected automatically.</p>
        </div>
      </section>
    );
  }

  const body = prettyBody(interaction);

  return (
    <section className="panel inspector-panel">
      <div className="inspector-head">
        <div>
          <div className="request-line">
            <span className={`method method-${interaction.method.toLowerCase()}`}>
              {interaction.method}
            </span>
            <strong>{interaction.path}</strong>
            {interaction.query ? <span className="query">?{interaction.query}</span> : null}
          </div>
          <div className="inspector-sub">
            <code>{interaction.interaction_id}</code>
            <span>sequence {interaction.sequence}</span>
          </div>
        </div>
        <button
          className="button ghost compact"
          type="button"
          onClick={() => onCopy(body, "body")}
        >
          {copied === "body" ? "Copied" : "Copy body"}
        </button>
      </div>

      <div className="tabs">
        <Tab active={tab === "body"} onClick={() => onTab("body")}>
          Body
        </Tab>
        <Tab active={tab === "headers"} onClick={() => onTab("headers")}>
          Headers <span>{interaction.headers.length}</span>
        </Tab>
        <Tab active={tab === "metadata"} onClick={() => onTab("metadata")}>
          Metadata
        </Tab>
      </div>

      <div className="inspector-content">
        {tab === "body" ? (
          <pre className="body-view" data-encoding={interaction.body_encoding}>
            {body || "∅"}
          </pre>
        ) : null}

        {tab === "headers" ? (
          <div className="kv-table">
            {interaction.headers.map(([name, value], index) => (
              <div className="kv-row" key={`${name}-${index}`}>
                <code>{name}</code>
                <code>{value}</code>
              </div>
            ))}
          </div>
        ) : null}

        {tab === "metadata" ? (
          <div className="kv-table">
            <KeyValue label="Interaction ID" value={interaction.interaction_id} />
            <KeyValue label="Exposure ID" value={interaction.exposure_id} />
            <KeyValue label="Sequence" value={String(interaction.sequence)} />
            <KeyValue
              label="Received"
              value={new Date(interaction.received_at_unix_ms).toISOString()}
            />
            <KeyValue label="Body bytes" value={String(interaction.body_bytes)} />
            <KeyValue label="Body encoding" value={interaction.body_encoding} />
          </div>
        ) : null}
      </div>
    </section>
  );
}

function Tab({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button className={active ? "tab active" : "tab"} type="button" onClick={onClick}>
      {children}
    </button>
  );
}

function KeyValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="kv-row">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function connectionLabel(state: ConnectionState): string {
  switch (state) {
    case "live":
      return "Live";
    case "connecting":
      return "Connecting";
    case "disconnected":
      return "Reconnecting";
    case "error":
      return "Stream error";
    default:
      return "Idle";
  }
}

function expiryLabel(unixSeconds?: number): string {
  if (!unixSeconds) return "—";
  const remaining = unixSeconds * 1000 - Date.now();
  if (remaining <= 0) return "expired";
  const hours = Math.ceil(remaining / 3_600_000);
  if (hours < 24) return `in ${hours}h`;
  return `in ${Math.ceil(hours / 24)}d`;
}

function shortId(value: string): string {
  return `${value.slice(0, 8)}…${value.slice(-4)}`;
}
