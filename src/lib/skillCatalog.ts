/** A skill in one of the curated collection repos (src/data/skill-catalog.json,
 *  built by scripts/build-skill-catalog.mjs). */
export interface CatalogSkill {
  /** `owner/repo` */
  repo: string;
  branch: string;
  /** Folder holding SKILL.md; "" when the repo root is the skill. */
  path: string;
  name: string;
  description: string;
}

let cached: Promise<CatalogSkill[]> | null = null;

/** The catalog, loaded on first use (it is ~1 MB, kept out of the main bundle). */
export function loadSkillCatalog(): Promise<CatalogSkill[]> {
  cached ??= import("../data/skill-catalog.json").then((m) => m.default as CatalogSkill[]);
  return cached;
}

export const catalogKey = (s: CatalogSkill) => `${s.repo}/${s.path}`;

/** The URL that installs exactly this skill through the git preview. */
export function catalogInstallUrl(s: CatalogSkill): string {
  return s.path
    ? `https://github.com/${s.repo}/tree/${s.branch}/${s.path}`
    : `https://github.com/${s.repo}`;
}

/** `owner/repo` + lowercased name → description, to fill in marketplace
 *  entries (skills.sh lists names only) that also appear in the catalog. */
export function descriptionIndex(catalog: CatalogSkill[]): Map<string, string> {
  const index = new Map<string, string>();
  for (const s of catalog) {
    if (!s.description) continue;
    const key = `${s.repo.toLowerCase()}/${s.name.toLowerCase()}`;
    if (!index.has(key)) index.set(key, s.description);
  }
  return index;
}

/** Case-insensitive match over name, description and repo; every word must hit. */
export function matchesCatalogQuery(s: CatalogSkill, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const hay = `${s.name} ${s.description} ${s.repo}`.toLowerCase();
  return words.every((w) => hay.includes(w));
}

let descriptions: Promise<Map<string, string>> | null = null;

/** `owner/repo/<skill id or name>` (lowercased) → description for skills on
 *  the skills.sh boards (src/data/skill-descriptions.json), merged with the
 *  collections catalog. Loaded on first use. */
export function loadLeaderboardDescriptions(): Promise<Map<string, string>> {
  descriptions ??= Promise.all([
    import("../data/skill-descriptions.json").then((m) => m.default as Record<string, string>),
    loadSkillCatalog(),
  ]).then(([board, catalog]) => {
    const index = descriptionIndex(catalog);
    for (const [key, value] of Object.entries(board)) index.set(key, value);
    return index;
  });
  return descriptions;
}

/** A leaderboard entry's description, by skill id first, then display name. */
export function leaderboardDescription(
  index: Map<string, string>,
  skill: { source: string; skill_id: string; name: string },
): string | undefined {
  const source = skill.source.toLowerCase();
  return index.get(`${source}/${skill.skill_id.toLowerCase()}`) ?? index.get(`${source}/${skill.name.toLowerCase()}`);
}
