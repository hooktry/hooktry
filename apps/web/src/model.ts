import type { Interaction } from "./types";

const VIEW_PATH = /^\/view\/(vw_[A-Za-z0-9_-]{32})\/?$/;
const HANDOFF_HASH = /^#(ho_[0-9a-f]{32})$/;

export function viewCapabilityFromPath(pathname: string): string | null {
  return pathname.match(VIEW_PATH)?.[1] ?? null;
}

export function handoffCapabilityFromHash(hash: string): string | null {
  return hash.match(HANDOFF_HASH)?.[1] ?? null;
}

export function viewSessionKey(capability: string): string {
  return `hooktry:web1:view:${capability}`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${trim(kib)} KiB`;
  return `${trim(kib / 1024)} MiB`;
}

export function formatExpiry(
  unixSeconds: number | undefined,
  nowMs = Date.now(),
): string {
  if (!unixSeconds) return "—";

  const remaining = unixSeconds * 1000 - nowMs;
  if (remaining <= 0) return "expired";

  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (remaining <= hour) {
    return `in ${Math.max(1, Math.ceil(remaining / minute))}m`;
  }
  if (remaining <= day) {
    return `in ${Math.ceil(remaining / hour)}h`;
  }
  return `in ${Math.ceil(remaining / day)}d`;
}

export function formatTimestamp(unixMs: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    fractionalSecondDigits: 3,
    hour12: false,
  }).format(new Date(unixMs));
}

export function prettyBody(interaction: Interaction): string {
  if (interaction.body_encoding === "hex") {
    return interaction.body;
  }

  const contentType = interaction.headers
    .find(([name]) => name.toLowerCase() === "content-type")?.[1]
    ?.toLowerCase();

  if (contentType?.includes("json")) {
    try {
      return JSON.stringify(JSON.parse(interaction.body), null, 2);
    } catch {
      return interaction.body;
    }
  }

  return interaction.body;
}

export function interactionMatches(
  interaction: Interaction,
  query: string,
): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;

  return [
    interaction.method,
    interaction.path,
    interaction.query ?? "",
    interaction.body,
    ...interaction.headers.flat(),
  ]
    .join("\n")
    .toLowerCase()
    .includes(normalized);
}

export function mergeInteraction(
  current: Interaction[],
  incoming: Interaction,
): Interaction[] {
  const index = current.findIndex(
    (item) => item.interaction_id === incoming.interaction_id,
  );
  if (index >= 0) {
    const next = current.slice();
    next[index] = incoming;
    return next.sort((a, b) => b.sequence - a.sequence);
  }
  return [incoming, ...current].sort((a, b) => b.sequence - a.sequence);
}

function trim(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1);
}
