import { AdapterError, sha256Hex } from "./core";
import {
  consumeHandoff,
  installHandoff,
  readHandoff,
} from "./repository";
import type {
  AnonymousOwnerProvision,
  Env,
} from "./types";

export const HANDOFF_TTL_SECONDS = 30 * 60;

const HANDOFF_AAD = new TextEncoder().encode("hooktry-handoff-v1");

export async function storeHandoff(
  env: Env,
  input: {
    exposureId: string;
    token: string;
    provision: AnonymousOwnerProvision;
    now: number;
  },
): Promise<number> {
  const expiresAt = input.now + HANDOFF_TTL_SECONDS;
  const sealed = await seal(input.token, input.provision);
  await installHandoff(env, {
    exposureId: input.exposureId,
    capabilityDigest: await sha256Hex(input.token),
    ciphertext: sealed.ciphertext,
    nonce: sealed.nonce,
    expiresAt,
    now: input.now,
  });
  return expiresAt;
}

export async function exchangeHandoff(
  env: Env,
  token: string,
  now: number,
): Promise<AnonymousOwnerProvision> {
  const digest = await sha256Hex(token);
  const stored = await readHandoff(env, digest, now);
  if (!stored) {
    throw new AdapterError(410, "invalid_handoff");
  }

  const provision = await unseal(
    token,
    stored.handoff_payload_ciphertext,
    stored.handoff_payload_nonce,
  );

  await consumeHandoff(env, {
    exposureId: stored.exposure_id,
    capabilityDigest: digest,
    now,
  });

  return provision;
}

async function seal(
  token: string,
  provision: AnonymousOwnerProvision,
): Promise<{ ciphertext: string; nonce: string }> {
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await handoffKey(token, ["encrypt"]);
  const plaintext = new TextEncoder().encode(JSON.stringify(provision));
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      {
        name: "AES-GCM",
        iv: nonce,
        additionalData: HANDOFF_AAD,
      },
      key,
      plaintext,
    ),
  );

  return {
    ciphertext: base64UrlEncode(ciphertext),
    nonce: base64UrlEncode(nonce),
  };
}

async function unseal(
  token: string,
  ciphertext: string,
  nonce: string,
): Promise<AnonymousOwnerProvision> {
  try {
    const key = await handoffKey(token, ["decrypt"]);
    const plaintext = await crypto.subtle.decrypt(
      {
        name: "AES-GCM",
        iv: base64UrlDecode(nonce),
        additionalData: HANDOFF_AAD,
      },
      key,
      base64UrlDecode(ciphertext),
    );
    return JSON.parse(
      new TextDecoder().decode(plaintext),
    ) as AnonymousOwnerProvision;
  } catch {
    throw new AdapterError(410, "invalid_handoff");
  }
}

async function handoffKey(
  token: string,
  usages: KeyUsage[],
): Promise<CryptoKey> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(token),
  );
  return crypto.subtle.importKey(
    "raw",
    digest,
    { name: "AES-GCM" },
    false,
    usages,
  );
}

function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
}

function base64UrlDecode(value: string): Uint8Array {
  const padded = value
    .replaceAll("-", "+")
    .replaceAll("_", "/")
    .padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(padded);
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}
