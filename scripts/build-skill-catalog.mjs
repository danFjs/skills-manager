// Builds src/data/skill-catalog.json: every skill (SKILL.md) in the repos
// listed in src/data/skill-repos.json, with its frontmatter name and
// description, so the Collections tab can browse skills offline.
//
//   GITHUB_TOKEN=$(gh auth token) node scripts/build-skill-catalog.mjs
//
// With `--descriptions <boards.json>` (a dump of the skills.sh boards, see
// `dump_leaderboards` in src-tauri/src/core/skillssh_api.rs) it instead
// scans the repos those skills come from and writes
// src/data/skill-descriptions.json: "owner/repo/name" → description, so the
// Leaderboard can show what each skill does.
//
// One GitHub API call per repo (recursive tree); file contents come from
// raw.githubusercontent.com, which does not use API quota. Without a token
// the API allows 60 calls/hour — enough for about half the list.
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const reposPath = join(root, "src", "data", "skill-repos.json");
const catalogPath = join(root, "src", "data", "skill-catalog.json");
const descriptionsPath = join(root, "src", "data", "skill-descriptions.json");
const boardsArg = process.argv.indexOf("--descriptions");
const boardsPath = boardsArg >= 0 ? process.argv[boardsArg + 1] : null;
const token = process.env.GITHUB_TOKEN;
const MAX_DESCRIPTION = 400;
const CONCURRENCY = 12;

const api = async (path) => {
  const res = await fetch(`https://api.github.com${path}`, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "skills-manager-catalog-builder",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
  });
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
};

/** name/description from YAML frontmatter: flat keys plus `>`/`|` blocks. */
function frontmatter(raw) {
  const lines = raw.replace(/\r\n/g, "\n").split("\n");
  if (lines[0]?.trim() !== "---") return {};
  const out = {};
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "---") break;
    const m = /^([A-Za-z_][\w-]*):\s*(.*)$/.exec(line);
    if (!m) continue;
    let [, key, value] = m;
    if (/^[>|][-+]?$/.test(value.trim())) {
      const body = [];
      while (i + 1 < lines.length && (lines[i + 1].startsWith(" ") || lines[i + 1].trim() === "")) {
        body.push(lines[++i].trim());
      }
      value = body.join(" ").trim();
    } else {
      value = value.trim().replace(/^(["'])(.*)\1$/, "$2");
    }
    if (key === "name" || key === "description") out[key] = value;
  }
  return out;
}

async function pool(items, worker) {
  const results = [];
  let next = 0;
  await Promise.all(
    Array.from({ length: CONCURRENCY }, async () => {
      while (next < items.length) {
        const i = next++;
        results[i] = await worker(items[i]).catch((e) => ({ error: String(e) }));
      }
    }),
  );
  return results;
}

const repos = boardsPath
  ? [...new Set(JSON.parse(await readFile(boardsPath, "utf8")).map((b) => b.source))]
      .filter((source) => /^[\w.-]+\/[\w.-]+$/.test(source))
      .map((repo) => ({ repo }))
  : JSON.parse(await readFile(reposPath, "utf8"));
const found = [];
const failed = [];
await pool(repos, async ({ repo }) => {
  const meta = await api(`/repos/${repo}`);
  const branch = meta.default_branch;
  const tree = await api(`/repos/${repo}/git/trees/${encodeURIComponent(branch)}?recursive=1`);
  const manifests = tree.tree.filter(
    (e) => e.type === "blob" && /(^|\/)skill\.md$/i.test(e.path) && !/(^|\/)(node_modules|\.git)\//.test(e.path),
  );
  for (const m of manifests) found.push({ repo, branch, file: m.path });
  return manifests.length;
}).then((counts) =>
  counts.forEach((c, i) => {
    if (c && c.error) failed.push(`${repos[i].repo}: ${c.error}`);
  }),
);
console.log(`${found.length} SKILL.md files in ${repos.length - failed.length} repos`);

const skills = (
  await pool(found, async ({ repo, branch, file }) => {
    const url = `https://raw.githubusercontent.com/${repo}/${encodeURIComponent(branch)}/${file
      .split("/")
      .map(encodeURIComponent)
      .join("/")}`;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const fm = frontmatter(await res.text());
    const path = file.includes("/") ? file.slice(0, file.lastIndexOf("/")) : "";
    const folder = path.split("/").pop() || repo.split("/")[1];
    return {
      repo,
      branch,
      path,
      name: (fm.name || folder).trim(),
      description: (fm.description || "").slice(0, MAX_DESCRIPTION).trim(),
    };
  })
).filter((s) => s && !s.error);

if (boardsPath) {
  // Keyed by both the frontmatter name and the folder name: skills.sh ids
  // are usually the folder, display names usually the frontmatter name.
  const index = {};
  for (const s of skills) {
    if (!s.description) continue;
    const folder = s.path.split("/").pop() || s.repo.split("/")[1];
    for (const n of new Set([s.name, folder])) {
      const key = `${s.repo}/${n}`.toLowerCase();
      if (!(key in index)) index[key] = s.description;
    }
  }
  // Keep only what the boards actually list — the scanned repos hold far more.
  const wanted = new Set(
    JSON.parse(await readFile(boardsPath, "utf8")).flatMap((b) =>
      [b.skill_id, b.name].filter(Boolean).map((n) => `${b.source}/${n}`.toLowerCase()),
    ),
  );
  for (const key of Object.keys(index)) if (!wanted.has(key)) delete index[key];
  await writeFile(descriptionsPath, JSON.stringify(index) + "\n");
  console.log(`${Object.keys(index).length} description keys from ${skills.length} skills → ${descriptionsPath}`);
} else {
  // One entry per repo + name + description: repos often ship the same skill
  // once per agent folder. Keep the shortest path.
  const best = new Map();
  for (const s of skills) {
    const key = `${s.repo.toLowerCase()}\u0001${s.name.toLowerCase()}\u0001${s.description}`;
    const have = best.get(key);
    if (!have || s.path.length < have.path.length || (s.path.length === have.path.length && s.path < have.path)) {
      best.set(key, s);
    }
  }
  const catalog = [...best.values()].sort(
    (a, b) => a.name.localeCompare(b.name) || a.repo.localeCompare(b.repo),
  );
  await writeFile(catalogPath, JSON.stringify(catalog) + "\n");
  console.log(`${skills.length} skills read, ${catalog.length} after dedupe → ${catalogPath}`);
  if (failed.length) console.log(`skipped repos:\n  ${failed.join("\n  ")}`);
}
