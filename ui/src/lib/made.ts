// Meals made (server: crates/core/src/made.rs). A dinner made with a protein keeps dinners with that
// protein out of the home page's suggestions for the day it was made and the next DAYS days.

export interface Made { at: number; path: string; title: string; tags: string[] }

export const PROTEINS = ['Chicken', 'Beef', 'Pork', 'Turkey', 'Seafood', 'Eggs', 'Beans'];
export const DAYS = 3;

/** Local calendar day as a whole number (unaffected by daylight-saving changes). */
const day = (ms: number) => { const d = new Date(ms); return Math.round(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 864e5); };
const has = (tags: string[], t: string) => tags.some((x) => x.toLowerCase() === t.toLowerCase());

/** The proteins of a recipe's tags, spelled as in PROTEINS. */
export const proteinsOf = (tags: string[]) => PROTEINS.filter((p) => has(tags, p));

/** Proteins not to suggest at `now`: those of dinners made in the last DAYS days. */
export function blockedProteins(made: Made[], now = Date.now()): string[] {
  const today = day(now);
  const recent = made.filter((m) => has(m.tags, 'Dinner') && today - day(m.at * 1000) <= DAYS && day(m.at * 1000) <= today);
  return PROTEINS.filter((p) => recent.some((m) => has(m.tags, p)));
}

export const madeToday = (made: Made[], path: string, now = Date.now()) => made.some((m) => m.path === path && day(m.at * 1000) === day(now));
