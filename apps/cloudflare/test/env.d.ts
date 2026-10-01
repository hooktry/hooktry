declare module "cloudflare:workers" {
  interface ProvidedEnv {
    ASSETS: Fetcher;
    DB: D1Database;
    PAYLOADS: R2Bucket;
    EXPOSURES: DurableObjectNamespace;
    CLAIM_INTERNAL_TOKEN?: string;
    GITHUB_CLIENT_ID?: string;
    GITHUB_CLIENT_SECRET?: string;
    USAGE_INGEST_TOKEN?: string;
    TEST_SCHEMA_STATEMENTS: string;
  }
}
