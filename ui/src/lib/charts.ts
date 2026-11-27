// Kitchen charts (#/charts): measure equivalents, oven temperatures, safe internal temperatures and an
// amount converter. Built from the same unit tables and rounding the recipes use (scale.ts), so a chart
// never disagrees with a scaled or Metric recipe.
import { UNIT_TO_ML, UNIT_TO_G, parseNum, formatNum, fToC } from './scale';

const FRACTIONS: [number, string][] = [[0, ''], [1 / 8, '⅛'], [1 / 4, '¼'], [1 / 3, '⅓'], [3 / 8, '⅜'], [1 / 2, '½'], [5 / 8, '⅝'], [2 / 3, '⅔'], [3 / 4, '¾'], [7 / 8, '⅞'], [1, '']];

/** An amount you can measure exactly with kitchen measures ("10⅔", "¼"), or null when it isn't one ("0.17"). */
export function clean(x: number): string | null {
  const w = Math.floor(x + 1e-9);
  const hit = FRACTIONS.find(([v]) => Math.abs(v - (x - w)) < 0.02);
  if (!hit) return null;
  const whole = hit[0] === 1 ? w + 1 : w;
  return (whole ? String(whole) : '') + hit[1] || null;
}

const metric = (v: number) => (v >= 10 ? String(Math.round(v)) : String(Math.round(v * 10) / 10));

// ---------- measure equivalents ----------

export const VOLUME_COLS = ['tsp', 'tbsp', 'cup', 'fl oz', 'ml'] as const;
const VOLUME_ROWS: [string, number, string][] = [
  ['1 tsp', 1, 'tsp'], ['1 tbsp', 1, 'tbsp'],
  ['⅛ cup', 1 / 8, 'cup'], ['¼ cup', 1 / 4, 'cup'], ['⅓ cup', 1 / 3, 'cup'], ['½ cup', 1 / 2, 'cup'],
  ['⅔ cup', 2 / 3, 'cup'], ['¾ cup', 3 / 4, 'cup'], ['1 cup', 1, 'cup'],
  ['1 pint', 1, 'pt'], ['1 quart', 1, 'qt'], ['1 gallon', 1, 'gal'],
];

/** One row per common measure; a cell is blank where nobody would measure that way (⅙ fl oz, 96 tsp). */
export function volumeRows(): { label: string; cells: string[] }[] {
  return VOLUME_ROWS.map(([label, n, unit]) => {
    const ml = n * UNIT_TO_ML[unit];
    return {
      label,
      cells: VOLUME_COLS.map((c) => {
        if (c === 'ml') return metric(ml);
        const v = ml / UNIT_TO_ML[c];
        return (c === 'tsp' || c === 'tbsp') && v > 48 ? '' : (clean(v) ?? '');
      }),
    };
  });
}

export const WEIGHT_COLS = ['oz', 'lb', 'g'] as const;
const WEIGHT_ROWS: [string, number, string][] = [
  ['1 oz', 1, 'oz'], ['4 oz', 4, 'oz'], ['8 oz', 8, 'oz'], ['12 oz', 12, 'oz'], ['1 lb', 1, 'lb'], ['2 lb', 2, 'lb'], ['1 kg', 1, 'kg'],
];
export function weightRows(): { label: string; cells: string[] }[] {
  return WEIGHT_ROWS.map(([label, n, unit]) => {
    const g = n * UNIT_TO_G[unit];
    return { label, cells: WEIGHT_COLS.map((c) => (c === 'g' ? metric(g) : (clean(g / UNIT_TO_G[c]) ?? formatNum(g / UNIT_TO_G[c])))) };
  });
}

// ---------- temperatures ----------

/** Oven settings 250–500°F, in °C the way a Metric recipe shows them. */
export const ovenRows = () => Array.from({ length: 11 }, (_, i) => 250 + i * 25).map((f) => ({ f, c: fToC(f) }));

/** USDA Food Safety and Inspection Service, safe minimum internal temperatures. */
export const SAFE_TEMPS_SOURCE = 'https://www.fsis.usda.gov/food-safety/safe-food-handling-and-preparation/food-safety-basics/safe-temperature-chart';
export const SAFE_TEMPS = ([
  { food: 'Beef, pork, veal and lamb: steaks, roasts and chops', f: 145, rest: 'Rest 3 minutes' },
  { food: 'Ground beef, pork, veal and lamb', f: 160 },
  { food: 'Chicken and turkey: whole, pieces and ground', f: 165 },
  { food: 'Stuffing cooked in the bird', f: 165 },
  { food: 'Ham, fresh or raw', f: 145, rest: 'Rest 3 minutes' },
  { food: 'Ham, fully cooked, to reheat', f: 140 },
  { food: 'Fish and shellfish', f: 145 },
  { food: 'Egg dishes', f: 160 },
  { food: 'Leftovers and casseroles', f: 165 },
] as { food: string; f: number; rest?: string }[]).map((r) => ({ ...r, c: fToC(r.f) }));

// ---------- the converter ----------

export const CONVERT_UNITS = {
  Volume: ['tsp', 'tbsp', 'fl oz', 'cup', 'pt', 'qt', 'gal', 'ml', 'l'],
  Weight: ['oz', 'lb', 'g', 'kg'],
  Temperature: ['°F', '°C'],
} as const;

/** The amount typed ("1 ½", "⅓", "1/3", "2.5") in every other unit of the same kind. Volume never turns into
 * weight: that needs the ingredient. */
export function convert(text: string, unit: string): { unit: string; value: string; about?: boolean }[] {
  const n = parseNum(text.replace(/\s+/g, ' '));
  if (!isFinite(n) || (n <= 0 && !unit.startsWith('°'))) return [];
  if (unit === '°F') return [{ unit: '°C', value: String(fToC(n)) }];
  if (unit === '°C') return [{ unit: '°F', value: String(Math.round((n * 9) / 5 + 32)) }];
  const table = UNIT_TO_ML[unit] !== undefined ? UNIT_TO_ML : UNIT_TO_G[unit] !== undefined ? UNIT_TO_G : null;
  if (!table) return [];
  const base = n * table[unit];
  const kind = table === UNIT_TO_ML ? CONVERT_UNITS.Volume : CONVERT_UNITS.Weight;
  return kind.filter((u) => u !== unit).flatMap((u): { unit: string; value: string; about?: boolean }[] => {
    const v = base / table[u];
    if (u === 'ml' || u === 'g') return [{ unit: u, value: metric(v) }];
    if (u === 'l' || u === 'kg') return v < 0.01 ? [] : [{ unit: u, value: String(Math.round(v * 100) / 100) }];
    const c = clean(v);
    // under 1 and not a kitchen fraction (⅓ cup = ⅙ pt): nobody measures that way, and "about ⅛" would mislead
    if (c === null && v < 1) return [];
    return [c ? { unit: u, value: c } : { unit: u, value: formatNum(v), about: true }];
  });
}
