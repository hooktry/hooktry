import { env } from "cloudflare:workers";
import { applyD1Migrations } from "cloudflare:test";

import type { Env } from "../src/types";

const testEnv = env as unknown as Env & { TEST_MIGRATIONS: D1Migration[] };

await applyD1Migrations(testEnv.DB, testEnv.TEST_MIGRATIONS);
