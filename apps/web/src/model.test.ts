import { describe, expect, it } from "vitest";

import {
  formatBytes,
  formatExactTimestamp,
  formatExpiry,
  formatRelativeTimestamp,
  handoffCapabilityFromHash,
  interactionMatches,
  mergeInteraction,
  prettyBody,
  viewCapabilityFromPath,
} from "./model";
import type { Interaction } from "./types";

const interaction: Interaction = {
  interaction_id: "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b",
  exposure_id: "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5c",
  sequence: 1,
  received_at_unix_ms: 1_780_000_000_000,
  method: "POST",
  path: "/stripe",
  query: "delivery=42",
  headers: [["content-type", "application/json"]],
  body_encoding: "utf8",
  body: '{"type":"checkout.session.completed"}',
  body_bytes: 37,
};

describe("WEB1 model", () => {
  it("recognizes only canonical view capability routes", () => {
    expect(
      viewCapabilityFromPath(
        "/view/vw_12345678901234567890123456789012",
      ),
    ).toBe("vw_12345678901234567890123456789012");
    expect(viewCapabilityFromPath("/hook/hk_123")).toBeNull();
    expect(viewCapabilityFromPath("/view/vw_short")).toBeNull();
  });

  it("recognizes only canonical fragment handoff capabilities", () => {
    expect(
      handoffCapabilityFromHash(
        "#ho_1234567890abcdef1234567890abcdef",
      ),
    ).toBe("ho_1234567890abcdef1234567890abcdef");
    expect(handoffCapabilityFromHash("#ho_short")).toBeNull();
    expect(
      handoffCapabilityFromHash(
        "#hk_1234567890abcdef1234567890abcdef",
      ),
    ).toBeNull();
  });

  it("formats retained sizes densely", () => {
    expect(formatBytes(18)).toBe("18 B");
    expect(formatBytes(2048)).toBe("2.0 KiB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MiB");
  });

  it("formats interaction time relatively", () => {
    const now = 1_800_000_000_000;
    expect(formatRelativeTimestamp(now - 5_000, now)).toBe("5 seconds ago");
    expect(formatRelativeTimestamp(now - 60_000, now)).toBe("1 minute ago");
    expect(formatRelativeTimestamp(now - 2 * 3_600_000, now)).toBe("2 hours ago");
    expect(formatRelativeTimestamp(now - 3 * 86_400_000, now)).toBe("3 days ago");
  });

  it("formats exact UTC timestamps with timezone context", () => {
    expect(formatExactTimestamp(1_800_000_000_000, "utc")).toMatch(/UTC/);
  });

  it("counts expiry down from days to hours to minutes", () => {
    const now = 1_800_000_000_000;
    expect(formatExpiry((now + 5 * 86_400_000) / 1000, now)).toBe("in 5d");
    expect(formatExpiry((now + 4 * 86_400_000) / 1000, now)).toBe("in 4d");
    expect(formatExpiry((now + 18 * 3_600_000) / 1000, now)).toBe("in 18h");
    expect(formatExpiry((now + 5 * 3_600_000) / 1000, now)).toBe("in 5h");
    expect(formatExpiry((now + 30 * 60_000) / 1000, now)).toBe("in 30m");
    expect(formatExpiry((now + 15 * 60_000) / 1000, now)).toBe("in 15m");
    expect(formatExpiry((now - 1) / 1000, now)).toBe("expired");
  });

  it("pretty-prints JSON and searches canonical evidence", () => {
    expect(prettyBody(interaction)).toContain(
      '"type": "checkout.session.completed"',
    );
    expect(interactionMatches(interaction, "stripe")).toBe(true);
    expect(interactionMatches(interaction, "delivery=42")).toBe(true);
    expect(interactionMatches(interaction, "not-present")).toBe(false);
  });

  it("deduplicates replayed backlog entries by Interaction id", () => {
    const updated = { ...interaction, body: "updated" };
    expect(mergeInteraction([interaction], updated)).toEqual([updated]);
  });
});
