import fs from "node:fs";

const databaseId = process.env.HOOKTRY_PREVIEW_D1_ID?.trim();
if (!databaseId) {
  throw new Error("HOOKTRY_PREVIEW_D1_ID is required");
}

const productionPath = new URL("../wrangler.production.jsonc", import.meta.url);
const outputPath = new URL("../wrangler.preview.generated.jsonc", import.meta.url);
const migrationPath = new URL("../wrangler.preview-migrations.generated.jsonc", import.meta.url);

const config = JSON.parse(fs.readFileSync(productionPath, "utf8"));

config.previews = {
  vars: {},
  d1_databases: [
    {
      binding: "DB",
      database_name: "hooktry-preview",
      database_id: databaseId,
    },
  ],
  r2_buckets: [
    {
      binding: "PAYLOADS",
      bucket_name: "hooktry-preview-payloads",
    },
  ],
  durable_objects: {
    bindings: [
      {
        name: "EXPOSURES",
        class_name: "ExposureRuntime",
      },
    ],
  },
  observability: {
    enabled: true,
  },
};

fs.writeFileSync(outputPath, JSON.stringify(config, null, 2) + "\n");
fs.writeFileSync(
  migrationPath,
  JSON.stringify(
    {
      d1_databases: [
        {
          binding: "PREVIEW_DB",
          database_name: "hooktry-preview",
          database_id: databaseId,
          migrations_dir: "migrations",
        },
      ],
    },
    null,
    2,
  ) + "\n",
);

console.log("Rendered isolated Preview bindings.");
