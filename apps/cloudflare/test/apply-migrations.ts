import { env as testEnv } from "cloudflare:workers";
import { applyD1Migrations } from "cloudflare:test";

import type { Env } from "../src/types";

type MigrationList = Parameters<typeof applyD1Migrations>[1];
const env = testEnv as unknown as Env & { TEST_MIGRATIONS: MigrationList };

await applyD1Migrations(env.DB, env.TEST_MIGRATIONS);
