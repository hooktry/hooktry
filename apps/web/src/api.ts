import type {
  AuthSession,
  ExposureSummary,
  HookProvision,
} from "./types";

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

export async function authSession(): Promise<AuthSession> {
  const response = await fetch(apiUrl("/api/v1/session"), {
    credentials: "same-origin",
    headers: { accept: "application/json" },
  });

  if (!response.ok) {
    const detail = await errorDetail(response);
    throw new Error(`Unable to read session (${response.status}): ${detail}`);
  }

  return (await response.json()) as AuthSession;
}

export function githubSignInUrl(returnTo: string): string {
  return apiUrl(
    `/api/v1/auth/github/start?return_to=${encodeURIComponent(returnTo)}`,
  );
}

export async function claimHook(
  claimUrl: string,
): Promise<ExposureSummary> {
  const response = await fetch(claimUrl, {
    method: "POST",
    credentials: "same-origin",
    headers: { accept: "application/json" },
  });

  if (!response.ok) {
    const detail = await errorDetail(response);
    throw new Error(`Unable to claim Hook (${response.status}): ${detail}`);
  }

  return (await response.json()) as ExposureSummary;
}

export async function logout(): Promise<void> {
  const response = await fetch(apiUrl("/api/v1/logout"), {
    method: "POST",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const detail = await errorDetail(response);
    throw new Error(`Unable to sign out (${response.status}): ${detail}`);
  }
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
