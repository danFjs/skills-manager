/** Page numbers to show around `current` (first, last, neighbours), as the
 *  Leaderboard has always done; gaps between them render as "…". */
export function visiblePages(total: number, current: number): number[] {
  return Array.from({ length: total }, (_, i) => i + 1).filter((page) => {
    if (total <= 7) return true;
    if (page === 1 || page === total) return true;
    return Math.abs(page - current) <= 1;
  });
}
