// Fetches the bundled embedding model (sentence-transformers/all-MiniLM-L6-v2,
// Apache-2.0) used to categorize skills. Pinned to an exact revision and
// verified by SHA-256, so every build ships identical bytes. The files are
// git-ignored; this runs automatically before `npm run dev` / `npm run build`
// and is a no-op once the verified files are present.
import { createHash } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "sentence-transformers/all-MiniLM-L6-v2";
const REVISION = "1110a243fdf4706b3f48f1d95db1a4f5529b4d41";
const FILES = {
  "config.json": "953f9c0d463486b10a6871cc2fd59f223b2c70184f49815e7efbcab5d8908b41",
  "tokenizer.json": "be50c3628f2bf5bb5e3a7f17b1f74611b2561a3a27eeab05e5aa30f411572037",
  "model.safetensors": "53aa51172d142c89d9012cce15ae4d6cc0ca6895895114379cacb4fab128d9db",
};

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "src-tauri", "resources", "minilm");

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function verified(path, expected) {
  try {
    return sha256(await readFile(path)) === expected;
  } catch {
    return false;
  }
}

await mkdir(dir, { recursive: true });
for (const [name, expected] of Object.entries(FILES)) {
  const path = join(dir, name);
  if (await verified(path, expected)) continue;
  const url = `https://huggingface.co/${REPO}/resolve/${REVISION}/${name}`;
  process.stdout.write(`fetching ${name}… `);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const actual = sha256(bytes);
  if (actual !== expected) {
    throw new Error(`${name}: checksum mismatch (expected ${expected}, got ${actual})`);
  }
  // write-then-rename so an interrupted download never looks complete
  await writeFile(`${path}.part`, bytes);
  await rename(`${path}.part`, path);
  console.log("ok");
}
