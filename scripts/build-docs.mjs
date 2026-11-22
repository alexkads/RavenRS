// Build the Astro + Starlight portal and merge it with the site's static applications.
import { cp, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const root = fileURLToPath(new URL("../", import.meta.url));
const out = resolve(process.argv[2] || resolve(root, "target/docs-preview"));
const result = spawnSync("npm", ["run", "build", "--prefix", "site"], {
  cwd: root, stdio: "inherit", env: process.env,
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status || 1);
await mkdir(out, { recursive: true });
await cp(resolve(root, "site/dist"), out, { recursive: true });
console.log(`Built Astro + Starlight portal into ${out}`);
