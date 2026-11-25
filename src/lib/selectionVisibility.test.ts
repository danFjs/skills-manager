import i18next from "i18next";
import { describe, expect, it } from "vitest";
import en from "../i18n/en.json";
import { countHiddenSelected, hiddenSelectionTexts } from "./selectionVisibility";

const ids = (...values: string[]) => new Set(values);

describe("countHiddenSelected", () => {
  it("hides nothing when there is no selection", () => {
    expect(countHiddenSelected(ids(), [])).toBe(0);
    expect(countHiddenSelected(ids(), ["a", "b"])).toBe(0);
  });

  it("hides nothing when every selected skill is rendered", () => {
    expect(countHiddenSelected(ids("a", "b"), ["a", "b", "c"])).toBe(0);
  });

  it("counts the skills of a collapsed source swept up by select-all", () => {
    // Three skills selected, only the one in the open source is rendered.
    expect(countHiddenSelected(ids("a", "b", "c"), ["a"])).toBe(2);
  });

  it("counts a source collapsed after its skills were picked", () => {
    // Same arithmetic, different cause: the selection was made row by row.
    const picked = ids("b", "c");
    expect(countHiddenSelected(picked, ["a"])).toBe(2);
  });

  it("hides nothing when grouping is off", () => {
    // Ungrouped, every filtered skill has a row.
    expect(countHiddenSelected(ids("a", "b", "c"), ["a", "b", "c"])).toBe(0);
  });

  it("hides nothing while a search forces every source open", () => {
    expect(countHiddenSelected(ids("a", "b"), ["a", "b", "c"])).toBe(0);
  });

  it("treats an id with no row as hidden, leaving pruning to the caller", () => {
    // `useMultiSelect` drops ids whose skill is gone before this runs, so a
    // missing row here can only mean a display reason, which is what we count.
    expect(countHiddenSelected(ids("gone"), [])).toBe(1);
  });

  it("counts each selected skill once even if a row repeats", () => {
    expect(countHiddenSelected(ids("a", "b"), ["a", "a"])).toBe(1);
  });
});

describe("hiddenSelectionTexts", () => {
  // The real strings: what is under test is which keys are used and that the
  // wording names no reason, so echoing keys back would prove nothing.
  async function textsIn(lng: string, selected: number, hidden: number) {
    const i18n = i18next.createInstance();
    await i18n.init({
      lng,
      resources: { en: { translation: en } },
      interpolation: { escapeValue: false },
    });
    return hiddenSelectionTexts(i18n.t.bind(i18n), selected, hidden);
  }

  it("says nothing extra when every selected skill is on screen", async () => {
    const texts = await textsIn("en", 3, 0);
    expect(texts.toolbarNote).toBeUndefined();
    expect(texts.deleteConfirm).toBe(
      "Delete 3 selected skills permanently? This removes their central library files, preset links, and synced agent targets."
    );
  });

  it("states how many are not shown without claiming why", async () => {
    // A filter can hide a selected skill as well as a collapsed Source.
    const en = await textsIn("en", 2, 2);
    expect(en.toolbarNote).toBe("(2 not shown)");
    expect(en.deleteConfirm).toBe(
      "Delete 2 selected skills permanently, including 2 not shown in the list? This removes their central library files, preset links, and synced agent targets."
    );

    expect(`${en.toolbarNote} ${en.deleteConfirm}`).not.toMatch(/collapsed/);
  });
});
