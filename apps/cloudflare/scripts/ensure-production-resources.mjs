import fs from "node:fs/promises";

const token = required("CLOUDFLARE_API_TOKEN");
const accountId = required("CLOUDFLARE_ACCOUNT_ID");

const D1_NAME = "ortyo-cloudflare";
const R2_NAME = "ortyo-payloads";
const CONFIG_PATH = "wrangler.production.generated.jsonc";

const d1List = await cf(`/accounts/${accountId}/d1/database?per_page=100`);
let database = d1List.result.find((item) => item.name === D1_NAME);

if (!database) {
  const created = await cf(`/accounts/${accountId}/d1/database`, {
    method: "POST",
    body: {
      name: D1_NAME,
      primary_location_hint: "eeur",
      read_replication: { mode: "disabled" },
    },
  });
  database = created.result;
  console.log(`Created D1 database ${D1_NAME}`);
} else {
  console.log(`Using existing D1 database ${D1_NAME}`);
}

const r2List = await cf(`/accounts/${accountId}/r2/buckets`);
let bucket = (r2List.result?.buckets ?? []).find((item) => item.name === R2_NAME);

if (!bucket) {
  const created = await cf(`/accounts/${accountId}/r2/buckets`, {
    method: "POST",
    body: {
      name: R2_NAME,
      locationHint: "eeur",
      storageClass: "Standard",
    },
  });
  bucket = created.result;
  console.log(`Created R2 bucket ${R2_NAME}`);
} else {
  console.log(`Using existing R2 bucket ${R2_NAME}`);
}

const subdomainResponse = await cf(`/accounts/${accountId}/workers/subdomain`);
const workersSubdomain = subdomainResponse.result?.subdomain;
if (!workersSubdomain) {
  throw new Error("Cloudflare Workers subdomain is not configured");
}

const template = JSON.parse(await fs.readFile("wrangler.jsonc", "utf8"));
template.account_id = accountId;
template.workers_dev = true;
template.d1_databases[0].database_id = database.uuid;
template.r2_buckets[0].bucket_name = bucket.name;

await fs.writeFile(CONFIG_PATH, JSON.stringify(template, null, 2) + "\n");

const baseUrl = `https://${template.name}.${workersSubdomain}.workers.dev`;
console.log(`Generated ${CONFIG_PATH}`);
console.log(`Worker URL: ${baseUrl}`);

await githubOutput("database_id", database.uuid);
await githubOutput("workers_subdomain", workersSubdomain);
await githubOutput("base_url", baseUrl);

function required(name) {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

async function cf(path, options = {}) {
  const response = await fetch(`https://api.cloudflare.com/client/v4${path}`, {
    method: options.method ?? "GET",
    headers: {
      authorization: `Bearer ${token}`,
      ...(options.body ? { "content-type": "application/json" } : {}),
    },
    body: options.body ? JSON.stringify(options.body) : undefined,
  });

  const payload = await response.json();
  if (!response.ok || payload.success === false) {
    const details = JSON.stringify(payload.errors ?? payload, null, 2);
    throw new Error(`Cloudflare API ${response.status} for ${path}: ${details}`);
  }
  return payload;
}

async function githubOutput(name, value) {
  const output = process.env.GITHUB_OUTPUT;
  if (!output) {
    return;
  }
  await fs.appendFile(output, `${name}=${value}\n`);
}
