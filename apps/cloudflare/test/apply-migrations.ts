import { env as testEnv } from "cloudflare:workers";
import { applyD1Migrations } from "cloudflare:test";

import type { Env } from "../src/types";

const env = testEnv as unknown as Env & { TEST_MIGRATIONS: D1Migration[] };

await applyD1Migrations(env.DB, env.TEST_MIGRATIONS);
