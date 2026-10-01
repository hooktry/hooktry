import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

const root = path.dirname(fileURLToPath(import.meta.url));
const testSchema = fs.readFileSync(
  path.join(root, "migrations", "0001_anonymous.sql"),
  "utf8",
);

export default defineConfig({
  plugins: [
    cloudflareTest({
      wrangler: {
        configPath: path.join(root, "wrangler.jsonc"),
      },
      miniflare: {
        bindings: {
          CLAIM_INTERNAL_TOKEN: "test-internal-token",
          TEST_SCHEMA: testSchema,
        },
      },
    }),
  ],
  test: {
    setupFiles: ["./test/apply-migrations.ts"],
  },
});
