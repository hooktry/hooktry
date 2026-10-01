import { viewCapabilityFromPath, viewSessionKey } from "./model";
import type { HookProvision } from "./types";

export function saveOwnerProvision(provision: HookProvision): void {
  const capability = viewCapabilityFromUrl(provision.view_url);
  if (!capability) return;

  try {
    sessionStorage.setItem(viewSessionKey(capability), JSON.stringify(provision));
  } catch {
    // Session persistence is a convenience only. Capabilities remain usable in-memory.
  }
}

export function loadOwnerProvision(pathname: string): HookProvision | null {
  const capability = viewCapabilityFromPath(pathname);
  if (!capability) return null;

  try {
    const raw = sessionStorage.getItem(viewSessionKey(capability));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as HookProvision;
    return viewCapabilityFromUrl(parsed.view_url) === capability ? parsed : null;
  } catch {
    return null;
  }
}

export function clearOwnerProvision(provision: HookProvision | null): void {
  if (!provision) return;
  const capability = viewCapabilityFromUrl(provision.view_url);
  if (!capability) return;

  try {
    sessionStorage.removeItem(viewSessionKey(capability));
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
