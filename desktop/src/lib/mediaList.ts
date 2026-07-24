/** Mirrors Swift `MediaSortOption` / `MediaStatusFilter` in MediaListViewModel.swift */

export type MediaSortOption =
  | "nameAscending"
  | "nameDescending"
  | "yearDescending"
  | "yearAscending"
  | "addedAtDescending"
  | "addedAtAscending"
  | "unscrapedFirst";

export type MediaStatusFilter =
  | "all"
  | "unscraped"
  | "scraped"
  | "partial"
  | "unmatched";

export const SORT_OPTIONS: { value: MediaSortOption; label: string }[] = [
  { value: "nameAscending", label: "Name (A-Z)" },
  { value: "nameDescending", label: "Name (Z-A)" },
  { value: "yearDescending", label: "Year (Newest First)" },
  { value: "yearAscending", label: "Year (Oldest First)" },
  { value: "addedAtDescending", label: "Recently Added" },
  { value: "addedAtAscending", label: "Earliest Added" },
  { value: "unscrapedFirst", label: "Unscraped First" },
];

export const STATUS_FILTERS: { value: MediaStatusFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "unscraped", label: "Unscraped" },
  { value: "scraped", label: "Scraped" },
  { value: "partial", label: "Incomplete" },
  { value: "unmatched", label: "Unmatched" },
];

export type SortableMedia = {
  id: string;
  title: string;
  originalTitle?: string | null;
  year?: number | null;
  status: string;
  addedAt: string;
};

function statusRank(status: string): number {
  switch (status) {
    case "unscraped":
      return 0;
    case "partial":
      return 1;
    case "unmatched":
      return 2;
    case "scraped":
      return 3;
    default:
      return 99;
  }
}

/** Approximate Swift folding + numeric compare for titles. */
function titleKey(title: string): string {
  return title.normalize("NFKD").replace(/\p{M}/gu, "").toLocaleLowerCase();
}

function compareTitle(a: string, b: string): number {
  return titleKey(a).localeCompare(titleKey(b), undefined, { numeric: true });
}

export function filterAndSortMedia<T extends SortableMedia>(
  items: T[],
  query: string,
  statusFilter: MediaStatusFilter,
  sortOption: MediaSortOption,
): T[] {
  const lowered = query.trim().toLowerCase();
  let next = items;
  if (lowered) {
    next = next.filter(
      (item) =>
        item.title.toLowerCase().includes(lowered) ||
        (item.originalTitle?.toLowerCase().includes(lowered) ?? false),
    );
  }
  if (statusFilter !== "all") {
    next = next.filter((item) => item.status === statusFilter);
  }

  const sorted = [...next];
  sorted.sort((lhs, rhs) => {
    switch (sortOption) {
      case "nameAscending":
        return compareTitle(lhs.title, rhs.title);
      case "nameDescending":
        return compareTitle(rhs.title, lhs.title);
      case "yearDescending": {
        const ly = lhs.year ?? Number.MIN_SAFE_INTEGER;
        const ry = rhs.year ?? Number.MIN_SAFE_INTEGER;
        if (ly === ry) return compareTitle(lhs.title, rhs.title);
        return ry - ly;
      }
      case "yearAscending": {
        const ly = lhs.year ?? Number.MAX_SAFE_INTEGER;
        const ry = rhs.year ?? Number.MAX_SAFE_INTEGER;
        if (ly === ry) return compareTitle(lhs.title, rhs.title);
        return ly - ry;
      }
      case "addedAtDescending": {
        if (lhs.addedAt === rhs.addedAt) return compareTitle(lhs.title, rhs.title);
        return lhs.addedAt < rhs.addedAt ? 1 : -1;
      }
      case "addedAtAscending": {
        if (lhs.addedAt === rhs.addedAt) return compareTitle(lhs.title, rhs.title);
        return lhs.addedAt > rhs.addedAt ? 1 : -1;
      }
      case "unscrapedFirst": {
        const lr = statusRank(lhs.status);
        const rr = statusRank(rhs.status);
        if (lr === rr) return compareTitle(lhs.title, rhs.title);
        return lr - rr;
      }
    }
  });
  return sorted;
}
