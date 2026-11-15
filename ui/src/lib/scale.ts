// Serving scaler + unit conversion for ingredient lines, working on the text as written.
// Unit tables and rounding ported from the old app (recipes/src/frontend/app.js:137-238).

const GLYPH: Record<string, number> = { '½': 1 / 2, '¼': 1 / 4, '¾': 3 / 4, '⅓': 1 / 3, '⅔': 2 / 3, '⅛': 1 / 8, '⅜': 3 / 8, '⅝': 5 / 8, '⅞': 7 / 8 };
const G = '½¼¾⅓⅔⅛⅜⅝⅞';
const NUM = `(?:\\d+\\s+\\d+\\s*[/⁄]\\s*\\d+|\\d+\\s*[${G}]|\\d+\\s*[/⁄]\\s*\\d+|\\d+(?:\\.\\d+)?|\\.\\d+|[${G}])`;
const QTY_RX = new RegExp(`^(\\s*(?:[*_]{1,2}|\\()?)(${NUM})(?:\\s*(?:-|–|to)\\s*(${NUM}))?`);

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

// ---------- yields ----------

const WORD_NUM: Record<string, number> = { one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9, ten: 10, eleven: 11, twelve: 12 };

export interface Yield { n: number; unit: string; dozen: boolean }

/**
 * A yield or servings value as a count: "6 servings", "Makes 2 dozen" (24, in dozens), "about five
 * 9” cakes", "12-15" (12). Null when it has no single count ("3 small, 2 large or 1 jumbo pizza").
 */
export function parseYield(v: unknown): Yield | null {
  if (typeof v === 'number') return v > 0 ? { n: v, unit: '', dozen: false } : null;
  if (typeof v !== 'string') return null;
  let s = v.trim().replace(/^\((.*)\)$/, '$1').trim();
  s = s.replace(/^(?:(?:makes|serves|servings|yields?|about|approx\.?|approximately)\s*:?\s+)+/i, '');
  const m = s.match(new RegExp(`^(${NUM}|${Object.keys(WORD_NUM).join('|')})(?![A-Za-z0-9])(?:\\s*(?:-|–|to)\\s*(?:${NUM})(?=\\s|$))?\\s*(.*)$`, 'i'));
  if (!m) return null;
  let n = m[1].toLowerCase() in WORD_NUM ? WORD_NUM[m[1].toLowerCase()] : parseNum(m[1]);
  let rest = m[2].trim();
  if (!(n > 0) || /\d/.test(rest)) return null; // a second number: no single count
  const dozen = /^dozen\b/i.test(rest);
  if (dozen) { n *= 12; rest = rest.replace(/^dozen\s*/i, ''); }
  return { n, unit: rest.replace(/^of\s+/i, '').replace(/[.,;]$/, '').trim(), dozen };
}

/** The scaler's label for a yield at a factor: "24 cookies (2 dozen)", "9 servings". */
export function yieldLabel(y: Yield, factor: number, fallbackUnit = ''): string {
  const n = y.n * factor;
  const unit = y.unit || fallbackUnit;
  if (y.dozen) return `${Math.round(n)}${unit ? ' ' + unit : ''} (${formatNum(n / 12)} dozen)`;
  return `${+n.toFixed(2)}${unit ? ' ' + unit : ''}`;
}

// ---------- tables with alternative portions ----------

const amountOf = (line: string): { v: number; unit: string } | null => {
  const m = line.match(QTY_RX);
  if (!m) return null;
  const v = parseNum(m[2]);
  const um = line.slice(m[0].length).match(UNIT_RX);
  return isFinite(v) && v > 0 ? { v, unit: um ? um[1].toLowerCase().replace(/\.$/, '').replace('fl. oz', 'fl oz') : '' } : null;
};
const inBase = (a: { v: number; unit: string }, b: { v: number; unit: string }): [number, number] | null => {
  for (const t of [UNIT_TO_ML, UNIT_TO_G]) if (t[a.unit] !== undefined && t[b.unit] !== undefined) return [a.v * t[a.unit], b.v * t[b.unit]];
  const same = (u: string) => u.replace(/e?s$/, '');
  return same(a.unit) === same(b.unit) ? [a.v, b.v] : null;
};
const snap = (x: number) => {
  const w = Math.floor(x);
  const parts = [0, 1 / 4, 1 / 3, 1 / 2, 2 / 3, 3 / 4, 1].map((f) => w + f);
  return parts.reduce((a, b) => (Math.abs(b - x) < Math.abs(a - x) ? b : a));
};
const LABEL_MULT: [RegExp, number][] = [[/\bhalf\b/i, 0.5], [/\b(single|one)\b/i, 1], [/\bdouble\b/i, 2], [/\btriple\b/i, 3], [/\bquadruple\b/i, 4]];

/**
 * How much each portion of a table makes compared with the first: the median ratio of its amounts
 * (matching or convertible units), snapped to a kitchen fraction; from the labels ("Double", "2x")
 * when the amounts can't tell. `null` for variants rather than sizes (Vanilla / Chocolate).
 */
export function portionMultipliers(items: { alts?: string[] }[], labels: string[]): (number | null)[] {
  const out: (number | null)[] = labels.map((label, k) => {
    if (k === 0) return 1;
    const ratios: number[] = [];
    for (const it of items) {
      const a = it.alts?.[0] ? amountOf(it.alts[0]) : null;
      const b = it.alts?.[k] ? amountOf(it.alts[k]) : null;
      const pair = a && b ? inBase(a, b) : null;
      if (pair) ratios.push(pair[1] / pair[0]);
    }
    if (ratios.length) {
      ratios.sort((x, y) => x - y);
      const h = ratios.length >> 1;
      const med = ratios.length % 2 ? ratios[h] : (ratios[h - 1] + ratios[h]) / 2;
      return med >= 0.9 && med <= 1.1 ? null : snap(med);
    }
    const n = label.match(/(\d+(?:\.\d+)?)\s*[x×]|[x×]\s*(\d+(?:\.\d+)?)/i);
    if (n) return +(n[1] ?? n[2]);
    return LABEL_MULT.find(([rx]) => rx.test(label))?.[1] ?? null;
  });
  // all variants: the first one is a variant too
  if (out.slice(1).every((m) => m === null)) out[0] = null;
  return out;
}

/**
 * Which portion to show, and how much to scale it: a size whose multiplier equals the factor is shown
 * as written (the family's own amounts); otherwise the chosen portion is scaled.
 */
export function resolvePortion(mults: (number | null)[], portion: number, factor: number): { col: number; scale: number } {
  const p = Math.min(portion, mults.length - 1);
  if (mults[p] !== null) {
    const exact = mults.findIndex((m) => m !== null && Math.abs(m - factor) < 1e-6);
    if (exact >= 0) return { col: exact, scale: 1 };
  }
  return { col: p, scale: factor / (mults[p] ?? 1) };
}

/** A group's ingredient lines at a portion and factor: a table shows that portion's amounts (lines it
 * doesn't use are left out); other groups are scaled as written. */
export function groupLines<L extends { line: number; text: string; alts?: string[] }>(
  g: { items: L[]; portions?: string[] }, portion: number, factor: number, units: Units,
): { line: number; text: string }[] {
  if (!g.portions?.length) return g.items.filter((it) => it.text).map((it) => ({ line: it.line, text: scaleLine(it.text, factor, units) }));
  const { col, scale } = resolvePortion(portionMultipliers(g.items, g.portions), portion, factor);
  return g.items
    .map((it) => ({ line: it.line, text: it.alts?.[col] ?? it.text }))
    .filter((it) => it.text)
    .map((it) => ({ line: it.line, text: scaleLine(it.text, scale, units) }));
}
