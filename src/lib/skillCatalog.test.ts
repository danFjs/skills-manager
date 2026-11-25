import { describe, expect, it } from "vitest";
import { catalogInstallUrl, descriptionIndex, matchesCatalogQuery, type CatalogSkill } from "./skillCatalog";

const skill = (over: Partial<CatalogSkill> = {}): CatalogSkill => ({
  repo: "acme/skills",
  branch: "main",
  path: "skills/pdf",
  name: "pdf",
  description: "Read and fill PDF forms",
  ...over,
});

describe("skillCatalog", () => {
  it("installs a single skill through a tree URL, or the repo when it is the root", () => {
    expect(catalogInstallUrl(skill())).toBe("https://github.com/acme/skills/tree/main/skills/pdf");
    expect(catalogInstallUrl(skill({ path: "" }))).toBe("https://github.com/acme/skills");
  });

  it("indexes descriptions by repo + name, case-insensitively, first wins", () => {
    const index = descriptionIndex([skill(), skill({ description: "later" }), skill({ name: "x", description: "" })]);
    expect(index.get("acme/skills/pdf")).toBe("Read and fill PDF forms");
    expect(index.has("acme/skills/x")).toBe(false);
  });

  it("requires every query word to match somewhere", () => {
    expect(matchesCatalogQuery(skill(), "pdf forms")).toBe(true);
    expect(matchesCatalogQuery(skill(), "acme pdf")).toBe(true);
    expect(matchesCatalogQuery(skill(), "pdf excel")).toBe(false);
    expect(matchesCatalogQuery(skill(), "  ")).toBe(true);
  });
});

describe("leaderboardDescription", () => {
  it("prefers the skill id and falls back to the display name", async () => {
    const { leaderboardDescription } = await import("./skillCatalog");
    const index = new Map([
      ["acme/skills/pdf-tools", "by id"],
      ["acme/skills/pdf", "by name"],
    ]);
    expect(leaderboardDescription(index, { source: "Acme/Skills", skill_id: "pdf-tools", name: "pdf" })).toBe("by id");
    expect(leaderboardDescription(index, { source: "acme/skills", skill_id: "other", name: "PDF" })).toBe("by name");
    expect(leaderboardDescription(index, { source: "x/y", skill_id: "a", name: "b" })).toBeUndefined();
  });
});
