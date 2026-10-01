import { env } from "cloudflare:workers";

import type { Env } from "../src/types";

const testEnv = env as unknown as Env & { TEST_SCHEMA: string };

await testEnv.DB.exec(testEnv.TEST_SCHEMA);
