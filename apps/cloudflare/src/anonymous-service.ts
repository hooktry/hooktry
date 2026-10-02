import {
  ANONYMOUS_TTL_SECONDS,
  anonymousPrincipal,
  randomCapability,
  randomPrincipal,
  sha256Hex,
  uuidV7,
} from "./core";
import { createExposure } from "./repository";
import type { AnonymousProvision, Env } from "./types";

export async function provisionAnonymousHook(
  request: Request,
  env: Env,
  principal = anonymousPrincipal(request) ?? randomPrincipal(),
): Promise<AnonymousProvision> {
  const hook = randomCapability("hk_");
  const view = randomCapability("vw_");
  const claim = randomCapability("cl_");
  const exposureId = uuidV7();
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + ANONYMOUS_TTL_SECONDS;

  const [
    principalDigest,
    ingressCapabilityDigest,
    viewCapabilityDigest,
    claimCapabilityDigest,
  ] = await Promise.all([
    sha256Hex(principal),
    sha256Hex(hook),
    sha256Hex(view),
    sha256Hex(claim),
  ]);

  const exposure = await createExposure(env, {
    exposureId,
    principalDigest,
    ingressCapabilityDigest,
    viewCapabilityDigest,
    claimCapabilityDigest,
    now,
    expiresAt,
  });

  const requestUrl = new URL(request.url);
  const publicUrl = new URL(
    env.HOOKTRY_PUBLIC_ORIGIN?.trim() || requestUrl.origin,
  );
  const base = publicUrl.origin;
  const websocketBase =
    publicUrl.protocol === "https:"
      ? `wss://${publicUrl.host}`
      : `ws://${publicUrl.host}`;

  return {
    ...exposure,
    hook_url: `${base}/hook/${hook}`,
    view_url: `${base}/view/${view}`,
    view_websocket_url: `${websocketBase}/view/${view}`,
    claim_url: `${base}/claim/${claim}`,
    anonymous_principal: principal,
  };
}
