import { env } from "cloudflare:workers";

import type { Env } from "../src/types";

const testEnv = env as unknown as Env & { TEST_SCHEMA_STATEMENTS: string };
const statements = JSON.parse(testEnv.TEST_SCHEMA_STATEMENTS) as string[];

await testEnv.DB.batch(
  statements.map((statement) => testEnv.DB.prepare(statement)),
);
