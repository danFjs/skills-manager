import { describe, expect, it } from "vitest";
import { visiblePages } from "./pagination";

describe("visiblePages", () => {
  it("shows every page when there are few", () => {
    expect(visiblePages(5, 3)).toEqual([1, 2, 3, 4, 5]);
  });
  it("keeps first, last and the current page's neighbours", () => {
    expect(visiblePages(25, 10)).toEqual([1, 9, 10, 11, 25]);
    expect(visiblePages(25, 1)).toEqual([1, 2, 25]);
    expect(visiblePages(25, 25)).toEqual([1, 24, 25]);
  });
});
