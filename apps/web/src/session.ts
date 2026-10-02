import { viewCapabilityFromPath, viewSessionKey } from "./model";
import type { HookProvision } from "./types";

export function saveOwnerProvision(provision: HookProvision): void {
  const capability = viewCapabilityFromUrl(provision.view_url);
  if (!capability) return;

  try {
    localStorage.setItem(viewSessionKey(capability), JSON.stringify(provision));
  } catch {
    try {
      sessionStorage.setItem(
        viewSessionKey(capability),
        JSON.stringify(provision),
      );
    } catch {
      // Persistence is a convenience only. Capabilities remain usable in-memory.
    }
  }
}

export function loadOwnerProvision(pathname: string): HookProvision | null {
  const capability = viewCapabilityFromPath(pathname);
  if (!capability) return null;

  try {
    const key = viewSessionKey(capability);
    const raw =
      localStorage.getItem(key) ??
      sessionStorage.getItem(key);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as HookProvision;
    if (viewCapabilityFromUrl(parsed.view_url) !== capability) {
      return null;
    }

    try {
      localStorage.setItem(key, raw);
      sessionStorage.removeItem(key);
    } catch {
      // Existing session storage remains a usable compatibility fallback.
    }
    return parsed;
  } catch {
    return null;
  }
}

export function clearOwnerProvision(provision: HookProvision | null): void {
  if (!provision) return;
  const capability = viewCapabilityFromUrl(provision.view_url);
  if (!capability) return;

  try {
    const key = viewSessionKey(capability);
    localStorage.removeItem(key);
    sessionStorage.removeItem(key);
  } catch {
    // Best-effort cleanup.
  }
}

function viewCapabilityFromUrl(value: string): string | null {
  try {
    return viewCapabilityFromPath(new URL(value, window.location.href).pathname);
  } catch {
    return null;
  }
}
