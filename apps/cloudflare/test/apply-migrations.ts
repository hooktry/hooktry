import { env as testEnv } from "cloudflare:workers";

import type { Env } from "../src/types";

const env = testEnv as unknown as Env & { TEST_SCHEMA_STATEMENTS: string };
const statements = JSON.parse(env.TEST_SCHEMA_STATEMENTS) as string[];

await env.DB.batch(
  statements.map((statement) => env.DB.prepare(statement)),
);
