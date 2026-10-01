import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

const root = path.dirname(fileURLToPath(import.meta.url));
const testSchemaStatements = fs
  .readdirSync(path.join(root, "migrations"))
  .filter((name) => name.endsWith(".sql"))
  .sort()
  .flatMap((name) =>
    fs
      .readFileSync(path.join(root, "migrations", name), "utf8")
      .split(";")
      .map((statement) => statement.trim())
      .filter(Boolean),
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
          GITHUB_CLIENT_ID: "test-github-client",
          GITHUB_CLIENT_SECRET: "test-github-secret",
          USAGE_INGEST_TOKEN: "test-usage-token",
          TEST_SCHEMA_STATEMENTS: JSON.stringify(testSchemaStatements),
        },
      },
    }),
  ],
  test: {
    setupFiles: ["./test/apply-migrations.ts"],
  },
});
