// Nutrition worked out from a recipe's ingredients: each line's amount (read by the server,
// `Line.amounts`) weighed in grams and multiplied by USDA's values per 100 g (`/api/foods`). An
// ingredient that is a recipe in the book is worked out from that recipe's own ingredients.
// Nothing is written to the recipe; lines that can't be weighed are left out and listed.
import { UNIT_TO_G, UNIT_TO_ML, parseYield, portionMultipliers, resolvePortion } from './scale';
import type { Amount, Entry } from './types';

/** In the order of the FDA Nutrition Facts label; the keys of `nutrition` in docs/FORMAT.md. */
export const NUTRIENTS = ['calories', 'fat', 'saturated_fat', 'trans_fat', 'cholesterol', 'sodium', 'carbohydrates', 'fiber', 'sugar', 'protein', 'vitamin_d', 'calcium', 'iron', 'potassium'] as const;
export type Nutrient = (typeof NUTRIENTS)[number];
export type Totals = Record<Nutrient, number>;

/** A USDA food: `n` per 100 g, `ml` grams per millilitre, `g` grams of one clove, stick, large… */
export interface Food { fdc: number; name: string; n: Partial<Totals>; ml?: number; g?: Record<string, number>; auto?: boolean }
/** By ingredient key; `null` = chosen not to count on the Ingredients page. */
export type Foods = Record<string, Food | null>;

/** Volumes the unit table leaves out: a pinch is 1/16 tsp, a dash ⅛ tsp (measuring-spoon sizes), a drop 0.05 ml. */
const SMALL_ML: Record<string, number> = { pinch: 4.929 / 16, dash: 4.929 / 8, drop: 0.05 };
const mlOf = (unit?: string) => (unit ? UNIT_TO_ML[unit] ?? SMALL_ML[unit] : undefined);

export interface Skipped { text: string; why: string }
/** One ingredient line for the ingredient statement: a food by its key, or a recipe of the book with
 * its own parts; `grams` null when it couldn't be weighed. */
export interface Part { key?: string; title?: string; grams: number | null; parts?: Part[] }
export interface Estimate {
  totals: Totals;
  /** ingredient lines counted, out of all ingredient lines (labels and directions are neither) */
  counted: number;
  lines: number;
  skipped: Skipped[];
  /** weight and volume of what was counted: how much of it a recipe that uses it takes */
  grams: number;
  ml: number;
  parts: Part[];
}

/** The recipe a line uses as an ingredient ("½ cup [mayonnaise](…)"), if any. */
export type SubRecipe = (text: string, from: Entry) => Entry | null;

const zero = () => Object.fromEntries(NUTRIENTS.map((k) => [k, 0])) as Totals;
function add(t: Totals, n: Partial<Totals>, f: number) {
  for (const k of NUTRIENTS) t[k] += (n[k] ?? 0) * f;
}

/** A line's weight in grams, or why it can't be weighed. */
export function gramsOf(a: Amount, food: Food): number | string {
  if (a.grams) return a.grams;
  if (a.qty == null) return 'no amount';
  const u = a.unit;
  if (u && UNIT_TO_G[u] !== undefined) return a.qty * UNIT_TO_G[u];
  const ml = mlOf(u);
  if (ml !== undefined) return food.ml ? a.qty * ml * food.ml : `no weight for a ${u}`;
  const g = food.g ?? {};
  // ponytail: an unsized item is USDA's medium (else large, whole, small); add per-food choices in foods-map.tsv `each`
  const each = u ? g[u] : g[a.size ?? ''] ?? g.each ?? g.medium ?? g.large ?? g.whole ?? g.small;
  return each ? a.qty * each : u ? `no weight for a ${u}` : 'no unit';
}

/** How much of a recipe a line uses: by weight or volume of what it makes, else a number of batches
 * (of its yield, when it has one). */
function shareOf(a: Amount, of: Estimate, r: Entry): number | string {
  if (a.qty == null && !a.grams) return 'no amount';
  const qty = a.qty ?? 1;
  const g = a.grams ?? (a.unit && UNIT_TO_G[a.unit] !== undefined ? qty * UNIT_TO_G[a.unit] : undefined);
  if (g !== undefined) return of.grams ? g / of.grams : `${r.title} can't be weighed`;
  const ml = mlOf(a.unit);
  if (ml !== undefined) return of.ml ? (qty * ml) / of.ml : `${r.title} can't be measured`;
  const y = parseYield(r.frontmatter?.servings) ?? parseYield(r.frontmatter?.yield);
  return qty / (y?.n ?? 1);
}

/**
 * The recipe's nutrition at a scale and portion (as the reader shows it), whole recipe. A table with
 * alternative portions counts the chosen portion's column, like the reader.
 * ponytail: every ingredient set in a file is added up, so a file holding two versions of a recipe
 * counts both; tell them apart if that turns out to matter.
 */
export function estimate(e: Entry, foods: Foods, sub: SubRecipe, factor = 1, portion = 0, seen: ReadonlySet<string> = new Set()): Estimate {
  const out: Estimate = { totals: zero(), counted: 0, lines: 0, skipped: [], grams: 0, ml: 0, parts: [] };
  if (!e.recipe) return out;
  const inside = new Set(seen).add(e.path);
  for (const g of e.recipe.ingredients) {
    const table = !!g.portions?.length;
    const { col, scale } = table ? resolvePortion(portionMultipliers(g.items, g.portions!), portion, factor) : { col: 0, scale: factor };
    for (const it of g.items) {
      const a = it.amounts?.[col];
      if (!a) continue; // a label, a direction, or not in this portion
      const text = (table ? it.alts?.[col] : it.text) || it.text;
      const unweighed = { key: a.key, grams: null };
      const skip = (why: string, part: Part | null = unweighed) => { out.skipped.push({ text, why }); if (part?.key || part?.title) out.parts.push(part); };
      out.lines++;
      if (a.skip) { skip(a.skip); continue; }
      const r = sub(text, e);
      if (r) {
        if (inside.has(r.path)) { skip(`${r.title} uses this recipe`, { title: r.title, grams: null }); continue; }
        const s = estimate(r, foods, sub, 1, 0, inside);
        const share = s.counted ? shareOf(a, s, r) : `nothing in ${r.title} could be counted`;
        if (typeof share === 'string') { skip(share, { title: r.title, grams: null, parts: s.parts }); continue; }
        out.parts.push({ title: r.title, grams: s.grams * share * scale, parts: s.parts });
        add(out.totals, s.totals, share * scale);
        out.grams += s.grams * share * scale;
        out.ml += s.ml * share * scale;
        out.counted++;
        for (const x of s.skipped) out.skipped.push({ text: `${r.title}: ${x.text}`, why: x.why });
        continue;
      }
      const food = a.key ? foods[a.key] : undefined;
      if (!food) { skip(food === null ? 'left out on the Ingredients page' : 'not in the nutrition table'); continue; }
      const grams = gramsOf(a, food);
      if (typeof grams === 'string') { skip(grams); continue; }
      add(out.totals, food.n, (grams / 100) * scale);
      out.parts.push({ key: a.key, grams: grams * scale });
      out.grams += grams * scale;
      // ponytail: a food with no USDA volume counts as water (1 g per ml) in a recipe used by the cup
      const ml = mlOf(a.unit);
      out.ml += (ml !== undefined && a.qty != null && !a.grams ? a.qty * ml : grams / (food.ml ?? 1)) * scale;
      out.counted++;
    }
  }
  return out;
}

/**
 * The ingredient statement as on a food label (21 CFR 101.4): each ingredient once, in descending order
 * of weight, a recipe of the book followed by its own ingredients in brackets (101.4(b)(2)).
 * Ingredients that couldn't be weighed follow in the recipe's order; `unweighed` counts them.
 * ponytail: no "contains 2% or less of" grouping (101.4(a)(2), optional); add it if labels get long.
 */
export function ingredientList(parts: Part[], name: (key: string) => string): { text: string; unweighed: number } {
  let unweighed = 0;
  const list = (ps: Part[]): string => {
    const one = new Map<string, Part>();
    for (const p of ps) {
      const id = p.key ? `k:${p.key}` : `r:${p.title}`;
      const had = one.get(id);
      if (!had) one.set(id, { ...p });
      else if (p.grams !== null) had.grams = (had.grams ?? 0) + p.grams;
    }
    const all = [...one.values()];
    const weighed = all.filter((p) => p.grams !== null).sort((x, y) => y.grams! - x.grams!);
    const rest = all.filter((p) => p.grams === null);
    unweighed += rest.length;
    return [...weighed, ...rest].map((p) => (p.key ? name(p.key) : p.parts?.length ? `${p.title} (${list(p.parts)})` : p.title!)).join(', ');
  };
  const text = list(parts);
  return { text, unweighed };
}

// ---------- the label ----------

/** FDA Daily Values, adults and children 4 years and over (21 CFR 101.9(c), 2016 rule). */
export const DAILY: Partial<Record<Nutrient, number>> = {
  fat: 78, saturated_fat: 20, cholesterol: 300, sodium: 2300, carbohydrates: 275, fiber: 28,
  vitamin_d: 20, calcium: 1300, iron: 18, potassium: 4700,
};
export const UNIT: Record<Nutrient, string> = {
  calories: '', fat: 'g', saturated_fat: 'g', trans_fat: 'g', cholesterol: 'mg', sodium: 'mg', carbohydrates: 'g',
  fiber: 'g', sugar: 'g', protein: 'g', vitamin_d: 'mcg', calcium: 'mg', iron: 'mg', potassium: 'mg',
};

const near = (v: number, step: number) => String(+(Math.round(v / step) * step).toFixed(1));

/** An amount as the label prints it, with the FDA's rounding rules (21 CFR 101.9(c)). */
export function labelAmount(k: Nutrient, v: number): string {
  switch (k) {
    case 'calories': return v < 5 ? '0' : near(v, v <= 50 ? 5 : 10);
    case 'fat': case 'saturated_fat': case 'trans_fat': return v < 0.5 ? '0' : near(v, v < 5 ? 0.5 : 1);
    case 'cholesterol': return v < 2 ? '0' : v < 5 ? 'less than 5' : near(v, 5);
    case 'sodium': case 'potassium': return v < 5 ? '0' : near(v, v <= 140 ? 5 : 10);
    case 'carbohydrates': case 'fiber': case 'sugar': case 'protein': return v < 0.5 ? '0' : v < 1 ? 'less than 1' : near(v, 1);
    case 'vitamin_d': case 'iron': return near(v, 0.1);
    case 'calcium': return near(v, 10);
  }
}

/** % Daily Value, whole percent; null where the label shows none. */
export function dailyValue(k: Nutrient, v: number): number | null {
  const d = DAILY[k];
  return d ? Math.round((v / d) * 100) : null;
}
