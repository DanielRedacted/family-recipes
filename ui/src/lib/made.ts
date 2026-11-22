// Meals made (server: crates/core/src/made.rs). A dinner made with a protein keeps dinners with that
// protein out of the home page's suggestions for the day it was made and the next DAYS days.
// A recipe kept on screen ASSUME_AFTER_MIN minutes without "Made today" is logged as `assumed`: shown
// as a guess on the Calendar page, where it can be confirmed or removed. Guesses never block a protein
// and never grey out the "Made today" button.

export interface Made { at: number; path: string; title: string; tags: string[]; by?: string; assumed?: boolean }

export const PROTEINS = ['Chicken', 'Beef', 'Pork', 'Turkey', 'Seafood', 'Eggs', 'Beans'];
export const DAYS = 3;
/** minutes a recipe stays on screen before it counts as probably cooked */
export const ASSUME_AFTER_MIN = 20;

/** Local calendar day as a whole number (unaffected by daylight-saving changes). */
export const day = (ms: number) => { const d = new Date(ms); return Math.round(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 864e5); };
const has = (tags: string[], t: string) => tags.some((x) => x.toLowerCase() === t.toLowerCase());

/** The proteins of a recipe's tags, spelled as in PROTEINS. */
export const proteinsOf = (tags: string[]) => PROTEINS.filter((p) => has(tags, p));

/** Proteins not to suggest at `now`: those of dinners made in the last DAYS days. */
export function blockedProteins(made: Made[], now = Date.now()): string[] {
  const today = day(now);
  const recent = made.filter((m) => !m.assumed && has(m.tags, 'Dinner') && today - day(m.at * 1000) <= DAYS && day(m.at * 1000) <= today);
  return PROTEINS.filter((p) => recent.some((m) => has(m.tags, p)));
}

/** Was `path` logged today? Guesses count only with `guesses` (so a guess isn't made twice). */
export const madeToday = (made: Made[], path: string, now = Date.now(), guesses = false) =>
  made.some((m) => m.path === path && (guesses || !m.assumed) && day(m.at * 1000) === day(now));
