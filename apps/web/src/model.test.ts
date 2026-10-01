import { describe, expect, it } from "vitest";

import {
  formatBytes,
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

  it("formats retained sizes densely", () => {
    expect(formatBytes(18)).toBe("18 B");
    expect(formatBytes(2048)).toBe("2.0 KiB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MiB");
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
