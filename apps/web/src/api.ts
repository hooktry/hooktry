import type { HookProvision } from "./types";

const configuredBase = (import.meta.env.VITE_ORTYO_API_BASE as string | undefined)
  ?.trim()
  .replace(/\/$/, "");

export async function createHook(): Promise<HookProvision> {
  const response = await fetch(apiUrl("/api/v1/hooks"), {
    method: "POST",
    credentials: "same-origin",
    headers: {
      accept: "application/json",
    },
  });

  if (!response.ok) {
    const detail = await errorDetail(response);
    throw new Error(`Unable to create Hook (${response.status}): ${detail}`);
  }

  return (await response.json()) as HookProvision;
}

export function websocketUrl(viewUrl: string): string {
  const url = new URL(viewUrl, window.location.href);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function apiUrl(path: string): string {
  return configuredBase ? `${configuredBase}${path}` : path;
}

async function errorDetail(response: Response): Promise<string> {
  try {
    const payload = (await response.json()) as {
      error?: { code?: string };
    };
    return payload.error?.code ?? response.statusText;
  } catch {
    return response.statusText || "request_failed";
  }
}
