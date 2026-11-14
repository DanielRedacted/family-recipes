// Serving scaler + unit conversion for ingredient lines, working on the text as written.
// Unit tables and rounding ported from the old app (recipes/src/frontend/app.js:137-238).

const GLYPH: Record<string, number> = { '½': 1 / 2, '¼': 1 / 4, '¾': 3 / 4, '⅓': 1 / 3, '⅔': 2 / 3, '⅛': 1 / 8, '⅜': 3 / 8, '⅝': 5 / 8, '⅞': 7 / 8 };
const G = '½¼¾⅓⅔⅛⅜⅝⅞';
const NUM = `(?:\\d+\\s+\\d+\\s*[/⁄]\\s*\\d+|\\d+\\s*[${G}]|\\d+\\s*[/⁄]\\s*\\d+|\\d+(?:\\.\\d+)?|\\.\\d+|[${G}])`;
const QTY_RX = new RegExp(`^(\\s*(?:\\*\\*)?)(${NUM})(?:\\s*(?:-|–|to)\\s*(${NUM}))?`);

const UNIT_TO_ML: Record<string, number> = {
  tsp: 4.929, ts: 4.929, teaspoon: 4.929, teaspoons: 4.929,
  tbsp: 14.787, tbs: 14.787, tb: 14.787, tablespoon: 14.787, tablespoons: 14.787,
  cup: 236.588, cups: 236.588, c: 236.588,
  'fl oz': 29.574, pt: 473.176, pint: 473.176, pints: 473.176,
  qt: 946.353, quart: 946.353, quarts: 946.353, gal: 3785.41, gallon: 3785.41, gallons: 3785.41,
  ml: 1, milliliter: 1, milliliters: 1, l: 1000, liter: 1000, liters: 1000, litre: 1000, litres: 1000,
};
const UNIT_TO_G: Record<string, number> = {
  oz: 28.3495, ounce: 28.3495, ounces: 28.3495, lb: 453.592, lbs: 453.592, pound: 453.592, pounds: 453.592,
  g: 1, gram: 1, grams: 1, kg: 1000, kilogram: 1000, kilograms: 1000,
};
const UNIT_RX = /^\s*(fl\.? oz|[A-Za-z]+)\.?(?=[\s,)]|$)/;

export type Units = 'original' | 'us' | 'metric';

export function parseNum(s: string): number {
  s = s.trim().replace('⁄', '/');
  let m = s.match(/^(\d+)\s+(\d+)\s*\/\s*(\d+)$/);
  if (m) return +m[1] + +m[2] / +m[3];
  m = s.match(/^(\d+)\s*([½¼¾⅓⅔⅛⅜⅝⅞])$/);
  if (m) return +m[1] + GLYPH[m[2]];
  m = s.match(/^(\d+)\s*\/\s*(\d+)$/);
  if (m) return +m[2] ? +m[1] / +m[2] : NaN;
  if (s in GLYPH) return GLYPH[s];
  return parseFloat(s);
}

/** Kitchen-friendly display: nearest of halves/thirds/quarters/eighths, with unicode glyphs. */
export function formatNum(x: number): string {
  if (!isFinite(x) || x <= 0) return String(x);
  const whole = Math.floor(x + 1e-9);
  const frac = x - whole;
  const opts: [number, string][] = [[0, ''], [1 / 8, '⅛'], [1 / 4, '¼'], [1 / 3, '⅓'], [3 / 8, '⅜'], [1 / 2, '½'], [5 / 8, '⅝'], [2 / 3, '⅔'], [3 / 4, '¾'], [7 / 8, '⅞'], [1, '']];
  let best = opts[0];
  for (const o of opts) if (Math.abs(o[0] - frac) < Math.abs(best[0] - frac)) best = o;
  const w = best[0] === 1 ? whole + 1 : whole;
  if (x >= 10 && best[1] && best[1] !== '½') return String(Math.round(x)); // no ⅛ precision above 10
  return (w > 0 ? String(w) : '') + best[1] || '0';
}

function convert(val: number, unit: string, to: Units): { val: number; unit: string; metric: boolean } | null {
  const u = unit.toLowerCase().replace(/\.$/, '').replace('fl. oz', 'fl oz');
  const ml = UNIT_TO_ML[u] !== undefined ? val * UNIT_TO_ML[u] : null;
  const g = ml === null && UNIT_TO_G[u] !== undefined ? val * UNIT_TO_G[u] : null;
  if (ml === null && g === null) return null;
  if (to === 'metric') {
    if (ml !== null) return ml >= 950 ? { val: ml / 1000, unit: 'L', metric: true } : { val: ml, unit: 'ml', metric: true };
    return g! >= 950 ? { val: g! / 1000, unit: 'kg', metric: true } : { val: g!, unit: 'g', metric: true };
  }
  if (ml !== null) {
    if (ml < 7.5) return { val: ml / 4.929, unit: 'tsp', metric: false };
    if (ml < 22) return { val: ml / 14.787, unit: 'tbsp', metric: false };
    if (ml < 2000) return { val: ml / 236.588, unit: 'cup', metric: false };
    return { val: ml / 3785.41, unit: 'gal', metric: false };
  }
  return g! < 340 ? { val: g! / 28.3495, unit: 'oz', metric: false } : { val: g! / 453.592, unit: 'lb', metric: false };
}

const metricFmt = (v: number, unit: string) => (unit === 'L' || unit === 'kg' ? (Math.round(v * 10) / 10).toString() : String(Math.round(v)));

/** Rewrite the leading quantity (and unit, when converting) of an ingredient line. */
export function scaleLine(text: string, factor: number, units: Units = 'original'): string {
  if (factor === 1 && units === 'original') return text;
  const m = text.match(QTY_RX);
  if (!m) return text;
  const a = parseNum(m[2]);
  const b = m[3] !== undefined ? parseNum(m[3]) : null;
  if (!isFinite(a) || (b !== null && !isFinite(b))) return text;
  let rest = text.slice(m[0].length);
  let fmtA = formatNum(a * factor);
  let fmtB = b !== null ? formatNum(b * factor) : null;
  if (units !== 'original') {
    const um = rest.match(UNIT_RX);
    if (um) {
      const ca = convert(a * factor, um[1], units);
      const cb = b !== null ? convert(b * factor, um[1], units) : null;
      if (ca && ca.unit.toLowerCase() !== um[1].toLowerCase().replace(/s$/, '')) {
        const f = (c: { val: number; unit: string; metric: boolean }) => (c.metric ? metricFmt(c.val, c.unit) : formatNum(c.val));
        fmtA = f(ca);
        fmtB = cb ? f(cb) : null;
        rest = ' ' + ca.unit + rest.slice(um[0].length);
      }
    }
  }
  const sep = b !== null ? (m[0].includes('to') ? ' to ' : '–') : '';
  return m[1] + fmtA + (fmtB !== null ? sep + fmtB : '') + rest;
}

/** Leading number of a yield/servings value ("6 servings", "12-15", 8). */
export function yieldNumber(v: unknown): number | null {
  if (typeof v === 'number') return v > 0 ? v : null;
  if (typeof v !== 'string') return null;
  const m = v.match(new RegExp(`^\\s*(?:about\\s+|makes\\s+)?(${NUM})`, 'i'));
  return m ? parseNum(m[1]) || null : null;
}
