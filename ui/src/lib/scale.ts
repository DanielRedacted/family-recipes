// Serving scaler + unit conversion for ingredient lines, working on the text as written.
import { normalizeUnits } from './units';
// Unit tables and rounding first ported from the earlier `recipes` app's frontend.

const GLYPH: Record<string, number> = { '½': 1 / 2, '¼': 1 / 4, '¾': 3 / 4, '⅓': 1 / 3, '⅔': 2 / 3, '⅛': 1 / 8, '⅜': 3 / 8, '⅝': 5 / 8, '⅞': 7 / 8 };
const G = '½¼¾⅓⅔⅛⅜⅝⅞';
const NUM = `(?:\\d+\\s+\\d+\\s*[/⁄]\\s*\\d+|\\d+\\s*[${G}]|\\d+\\s*[/⁄]\\s*\\d+|\\d+(?:\\.\\d+)?|\\.\\d+|[${G}])`;
const QTY_RX = new RegExp(`^(\\s*(?:[*_]{1,2}|\\()?)(${NUM})(?:\\s*(?:-|–|to)\\s*(${NUM}))?`);

export const UNIT_TO_ML: Record<string, number> = {
  tsp: 4.929, ts: 4.929, teaspoon: 4.929, teaspoons: 4.929,
  tbsp: 14.787, tbs: 14.787, tb: 14.787, tablespoon: 14.787, tablespoons: 14.787,
  cup: 236.588, cups: 236.588, c: 236.588,
  'fl oz': 29.574, pt: 473.176, pint: 473.176, pints: 473.176,
  qt: 946.353, quart: 946.353, quarts: 946.353, gal: 3785.41, gallon: 3785.41, gallons: 3785.41,
  ml: 1, milliliter: 1, milliliters: 1, l: 1000, liter: 1000, liters: 1000, litre: 1000, litres: 1000,
};
export const UNIT_TO_G: Record<string, number> = {
  oz: 28.3495, ounce: 28.3495, ounces: 28.3495, lb: 453.592, lbs: 453.592, pound: 453.592, pounds: 453.592,
  g: 1, gram: 1, grams: 1, kg: 1000, kilogram: 1000, kilograms: 1000,
};
const UNIT_RX = /^\s*(fl\.? oz|[A-Za-z]+)\.?(?=[\s,)]|$)/;

/** US: the recipe's own units, tidied when scaling makes an amount awkward; Metric: ml / g. */
export type Units = 'us' | 'metric';

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
  return (w > 0 ? String(w) : '') + best[1] || '⅛'; // a nonzero amount never reads as 0
}

/**
 * Things you can't measure part of: after scaling, a whole number, at least 1 ("3 large eggs" × ½ →
 * "2 large eggs"), but only when the recipe writes whole ones: "½ head cabbage" stays a fraction.
 * Deliberately a short list: "½ onion" or "1½ sticks butter" are real amounts.
 * Groups: 1 what comes before the noun (bold marks, "(15 oz)", size words), 2 the noun, 3 its plural ending.
 */
const COUNT_RX = /^([*_]{0,2}\s*(?:\([^)]*\)\s*)?(?:(?:extra[- ]large|large|medium|small|jumbo|whole|garlic)\s+)*)(egg\s+yolk|egg\s+white|yolk|egg|clove|can|slice|piece|head|package|bunch|sprig|stalk)(s|es)?\b/i;
const roundCount = (x: number) => String(Math.max(1, Math.round(x)));
/** The noun agrees with the rounded count: "1 eggs" → "1 egg", "2 clove" → "2 cloves", "3 EGGS". */
function countNoun(rest: string, n: number): string {
  const m = rest.match(COUNT_RX)!;
  let plural = n > 1 ? (/ch$/i.test(m[2]) ? 'es' : 's') : '';
  if (m[2] === m[2].toUpperCase()) plural = plural.toUpperCase();
  return m[1] + m[2] + plural + rest.slice(m[0].length);
}

/** Metric: millilitres / grams (litres / kilograms from 950 up). */
function toMetric(val: number, unit: string): { val: number; unit: string } | null {
  const u = unit.toLowerCase().replace(/\.$/, '').replace('fl. oz', 'fl oz');
  if (UNIT_TO_ML[u] !== undefined) { const ml = val * UNIT_TO_ML[u]; return ml >= 950 ? { val: ml / 1000, unit: 'l' } : { val: ml, unit: 'ml' }; }
  if (UNIT_TO_G[u] !== undefined) { const g = val * UNIT_TO_G[u]; return g >= 950 ? { val: g / 1000, unit: 'kg' } : { val: g, unit: 'g' }; }
  return null;
}

/** US units and the amounts each reads well in; a scaled amount outside its unit's range moves to the next unit. */
const LADDERS: { unit: string; size: number; from: number; to: number }[][] = [
  [
    { unit: 'tsp', size: 4.929, from: 0, to: 3 },
    { unit: 'tbsp', size: 14.787, from: 1, to: 4 },
    { unit: 'cup', size: 236.588, from: 0.25, to: 16 },
    { unit: 'gal', size: 3785.41, from: 1, to: Infinity },
  ],
  [
    { unit: 'oz', size: 28.3495, from: 0, to: 16 },
    { unit: 'lb', size: 453.592, from: 0.5, to: Infinity },
  ],
];

/**
 * US, after scaling: the recipe's own unit while the amount reads well in it (⅛ tsp up to 3 tsp,
 * 1 up to 4 tbsp, ¼ up to 16 cups, …), else the unit it does read well in ("16 tbsp" × ½ → "½ cup").
 * Other units (qt, pt, fl oz, g, ml…) keep their unit.
 */
function tidyUS(val: number, unit: string): { val: number; unit: string } | null {
  for (const ladder of LADDERS) {
    const own = ladder.find((s) => s.unit === unit);
    if (!own) continue;
    if (val >= (own.unit === 'tsp' ? 1 / 8 : own.from) && val < own.to) return null;
    const base = val * own.size;
    const fit = ladder.find((s) => base / s.size >= s.from && base / s.size < s.to);
    return fit && fit.unit !== unit ? { val: base / fit.size, unit: fit.unit } : null;
  }
  return null;
}

/** Whole ml / g; one decimal for litres, kilograms and anything under 1 (never "0 ml"). */
const metricFmt = (v: number, unit: string) =>
  unit === 'l' || unit === 'kg' || v < 1 ? String(Math.max(0.1, Math.round(v * 10) / 10)) : String(Math.round(v));
const unitName = (unit: string, val: number) => (unit === 'cup' && val > 1 ? 'cups' : unit);

const GLYPHS: Record<string, string> = { '1/2': '½', '1/4': '¼', '3/4': '¾', '1/3': '⅓', '2/3': '⅔', '1/8': '⅛', '3/8': '⅜', '5/8': '⅝', '7/8': '⅞' };
/** "1 1/2" → "1½", "3/4" → "¾": written kitchen fractions as glyphs, as scaled amounts read; nothing rounded
 * (1/16, 2.5 stay as written). */
const glyphs = (q: string) => q.replace(/(\d+\s+)?(?<!\d)(\d)\s*[/⁄]\s*(\d)(?!\d)/g,
  (all, whole: string | undefined, a: string, b: string) => (GLYPHS[a + '/' + b] ? (whole?.trim() ?? '') + GLYPHS[a + '/' + b] : all));

/** Rewrite the leading quantity (and unit, when converting) of an ingredient line; units in one spelling. */
export function scaleLine(text: string, factor: number, units: Units = 'us'): string {
  text = normalizeUnits(text);
  if (factor === 1 && units === 'us') return text.replace(QTY_RX, glyphs);
  const m = text.match(QTY_RX);
  if (!m) return text;
  const a = parseNum(m[2]);
  const b = m[3] !== undefined ? parseNum(m[3]) : null;
  if (!isFinite(a) || (b !== null && !isFinite(b))) return text;
  let rest = text.slice(m[0].length);
  let fmtA = formatNum(a * factor);
  let fmtB = b !== null ? formatNum(b * factor) : null;
  const um = rest.match(UNIT_RX);
  if (um) {
    const unit = um[1].replace(/^cups$/, 'cup');
    if (units === 'metric') {
      const ca = toMetric(a * factor, unit);
      const cb = b !== null ? toMetric(b * factor, unit) : null;
      if (ca) {
        fmtA = metricFmt(ca.val, ca.unit);
        fmtB = cb ? metricFmt(cb.val, ca.unit === cb.unit ? cb.unit : ca.unit) : null;
        rest = ' ' + ca.unit + rest.slice(um[0].length);
      }
    } else {
      const ta = tidyUS(a * factor, unit);
      if (ta) {
        const ratio = ta.val / (a * factor);
        fmtA = formatNum(ta.val);
        fmtB = b !== null ? formatNum(b * factor * ratio) : null;
        rest = ' ' + unitName(ta.unit, b !== null ? b * factor * ratio : ta.val) + rest.slice(um[0].length);
      } else if (unit === 'cup') {
        rest = ' ' + unitName('cup', (b ?? a) * factor) + rest.slice(um[0].length);
      }
    }
  }
  if (factor !== 1 && Number.isInteger(a) && (b === null || Number.isInteger(b)) && COUNT_RX.test(rest)) {
    fmtA = roundCount(a * factor);
    fmtB = b !== null ? roundCount(b * factor) : null;
    rest = countNoun(rest, +(fmtB ?? fmtA));
  }
  const sep = b !== null ? (m[0].includes('to') ? ' to ' : '–') : '';
  return m[1] + fmtA + (fmtB !== null ? sep + fmtB : '') + rest;
}

/** "1 ½ cups flour" → "**1 ½ cups** flour": the reader shows the leading amount (and its unit) in bold.
 * A line whose amount is already marked up is left as written. */
export function boldQty(text: string): string {
  const m = text.match(QTY_RX);
  if (!m || /[*_]/.test(m[1]) || /^\s*[*_]/.test(text.slice(m[0].length))) return text;
  const um = text.slice(m[0].length).match(UNIT_RX);
  const u = um?.[1].toLowerCase().replace('fl. oz', 'fl oz');
  const end = m[0].length + (um && u && (u in UNIT_TO_ML || u in UNIT_TO_G) ? um[0].length : 0);
  return `${m[1]}**${text.slice(m[1].length, end).trim()}**${text.slice(end)}`;
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
