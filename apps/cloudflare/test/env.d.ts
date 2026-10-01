declare module "cloudflare:workers" {
  interface ProvidedEnv {
    ASSETS: Fetcher;
    DB: D1Database;
    PAYLOADS: R2Bucket;
    EXPOSURES: DurableObjectNamespace;
    CLAIM_INTERNAL_TOKEN?: string;
    TEST_SCHEMA_STATEMENTS: string;
  }
}
