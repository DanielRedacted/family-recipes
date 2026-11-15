// One spelling for every measurement unit: "1 Tablespoon", "1 TB", "1 T", "1 Tbsp." all read
// "1 Tbsp". Only a unit right after an amount changes ("a tablespoon of" in a sentence stays), and
// only its spelling: amounts, words and order are kept. Also: temperatures with a scale read
// "350°F", and fraction suffixes go ("1/8th" -> "1/8"). Plain TypeScript with no imports, so the
// one-off script that normalizes a recipes folder (scripts/normalize-units.ts) can run it in Node.

const G = '½¼¾⅓⅔⅛⅜⅝⅞';
const GLYPH: Record<string, number> = { '½': 1 / 2, '¼': 1 / 4, '¾': 3 / 4, '⅓': 1 / 3, '⅔': 2 / 3, '⅛': 1 / 8, '⅜': 3 / 8, '⅝': 5 / 8, '⅞': 7 / 8 };
/** one amount: 2, 2.5, 1/2, 1 1/2, 1½, 1 ½, ½ */
const N = `(?:\\d+(?:[.,]\\d+)?(?:\\s+\\d+\\s*[/⁄]\\s*\\d+|\\s*[/⁄]\\s*\\d+)?(?:\\s*[${G}])?|[${G}])`;
/** an amount or a range of them: "1-2", "½ to 1", "2 or 3" */
const AMOUNT = `${N}(?:\\s*(?:-|–|to|or)\\s*${N})?`;
const ADJ = '(?:heaping|scant|level|rounded|slight|generous)';

/** written unit (lower case) -> the one spelling */
const UNITS: Record<string, string> = {
  ts: 'tsp', tsp: 'tsp', tsps: 'tsp', teaspoon: 'tsp', teaspoons: 'tsp',
  tb: 'Tbsp', tbs: 'Tbsp', tbsp: 'Tbsp', tbsps: 'Tbsp', tbl: 'Tbsp', tbps: 'Tbsp', tablespoon: 'Tbsp', tablespoons: 'Tbsp',
  cup: 'cup', cups: 'cup',
  oz: 'oz', ounce: 'oz', ounces: 'oz',
  lb: 'lb', lbs: 'lb', pound: 'lb', pounds: 'lb',
  pt: 'pt', pint: 'pt', pints: 'pt', qt: 'qt', qts: 'qt', quart: 'qt', quarts: 'qt',
  gal: 'gal', gallon: 'gal', gallons: 'gal',
  g: 'g', gm: 'g', gram: 'g', grams: 'g', kg: 'kg', kilogram: 'kg', kilograms: 'kg',
  ml: 'ml', milliliter: 'ml', milliliters: 'ml', millilitre: 'ml', millilitres: 'ml',
  liter: 'L', liters: 'L', litre: 'L', litres: 'L',
};

/** The one spelling for a written unit, or null when it is not a measurement unit. */
export function unitOf(word: string): string | null {
  const w = word.replace(/\.$/, '');
  // single letters: t = teaspoon, T = tablespoon, c = cup, L = litre; C is Celsius (not here)
  if (w === 't') return 'tsp';
  if (w === 'T') return 'Tbsp';
  if (w === 'c') return 'cup';
  if (w === 'L') return 'L';
  if (w.length === 1 && w !== 'g') return null;
  if (/^(fl\.?\s?oz|fluid\s+ounces?)$/i.test(w)) return 'fl oz';
  return UNITS[w.toLowerCase()] ?? null;
}

function amountValue(s: string): number {
  s = s.trim().replace('⁄', '/');
  let m = s.match(/^(\d+)\s+(\d+)\s*\/\s*(\d+)$/);
  if (m) return +m[1] + +m[2] / +m[3];
  m = s.match(/^(\d+)\s*([½¼¾⅓⅔⅛⅜⅝⅞])$/);
  if (m) return +m[1] + GLYPH[m[2]];
  m = s.match(/^(\d+)\s*\/\s*(\d+)$/);
  if (m) return +m[2] ? +m[1] / +m[2] : NaN;
  if (s in GLYPH) return GLYPH[s];
  return parseFloat(s.replace(',', '.'));
}

const UNIT_WORD = `(fl\\.?\\s?oz|fluid\\s+ounces?|[A-Za-z]+)(\\.)?`;
const UNIT_RX = new RegExp(
  `(?<![\\w/.⁄${G}])(${AMOUNT})(\\*{1,2})?(\\s*)((?:${ADJ}\\s+)?)${UNIT_WORD}(?![A-Za-z])((?:\\s+${UNIT_WORD}(?![A-Za-z]))?)`,
  'g',
);
const TEMP_RX = /(?<![\w.])(\d{2,}(?:\s*[-–]\s*\d{2,})?)\s*(?:°|º|degrees?|deg\.?)?\s*(F|C|Fahrenheit|Celsius)\b/g;
const SUFFIX_RX = /(?<![\w/])(\d+\s*[/⁄]\s*\d+)(?:ths?|st|nds?|rds?)\b/g;
/** parts of a note that are addresses or link names, never rewritten */
const PROTECT = /(\]\([^)\n]*\)|\[\[[^\]\n]*\]\]|<?https?:\/\/[^\s)>]+>?)/;

function normalizePart(t: string): string {
  t = t.replace(TEMP_RX, (_, n: string, scale: string) => `${n}°${scale[0].toUpperCase()}`);
  t = t.replace(/(?<=\d\s?)º/g, '°');
  t = t.replace(SUFFIX_RX, '$1');
  return t.replace(UNIT_RX, (...a: any[]) => {
    const [all, amount, bold = '', sp, adj, word, dot, twice, word2] = a as string[];
    const offset = a[a.length - 2] as number;
    const unit = unitOf(word);
    if (!unit) return all;
    const nums = amount.split(/\s*(?:-|–|to|or)\s*/);
    const last = amountValue(nums[nums.length - 1]);
    const name = unit === 'cup' && last > 1 ? 'cups' : unit;
    // "½ c cup": the same unit written twice counts once
    const again = word2 && unitOf(word2) === unit ? '' : twice;
    // "Tbsp." loses its period, unless it also ends the sentence ("Add 1 tsp." / "… 1 tsp. Mix")
    const after = t.slice(offset + all.length);
    // an ingredient line ("- 1 tsp. Salt", "| 2 lb. | Chicken") starts with an amount: a capital after it is a name
    const line = t.slice(t.lastIndexOf('\n', offset) + 1);
    const ingredient = new RegExp(`^\\s*(?:(?:[-*+]|\\d+[.)])\\s+|\\|\\s*)?\\**[\\d${G}]`).test(line);
    const rest = (again ?? '') + after; // what follows the unit
    const ends = !!dot && (/^\s*$/.test(rest.split('\n')[0]) || (!ingredient && /^\s+[A-Z]/.test(rest)));
    return `${amount}${bold}${sp || ' '}${adj}${name}${ends ? '.' : ''}${again}`;
  });
}

/** Text with every measurement unit after an amount in its one spelling (see the table above). */
export function normalizeUnits(text: string): string {
  return text.split(PROTECT).map((part, i) => (i % 2 ? part : normalizePart(part))).join('');
}
