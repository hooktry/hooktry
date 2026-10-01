import path from "node:path";
import { fileURLToPath } from "node:url";

import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { readD1Migrations } from "@cloudflare/vitest-plugin/config";
import { defineConfig } from "vitest/config";

const root = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    cloudflareTest(async () => ({
      wrangler: {
        configPath: path.join(root, "wrangler.jsonc"),
      },
      miniflare: {
        bindings: {
          CLAIM_INTERNAL_TOKEN: "test-internal-token",
          TEST_MIGRATIONS: await readD1Migrations(path.join(root, "migrations")),
        },
      },
    })),
  ],
  test: {
    setupFiles: ["./test/apply-migrations.ts"],
  },
});
