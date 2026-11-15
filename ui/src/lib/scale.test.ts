import { describe, expect, it } from 'vitest';
import { boldQty, formatNum, groupLines, parseNum, parseYield, portionMultipliers, resolvePortion, scaleLine, yieldLabel } from './scale';

// Ingredient tables with one column per portion, as the parser hands them over (row = the line's alternatives).
const table = (portions: string[], ...rows: string[][]) =>
  ({ portions, items: rows.map((alts, i) => ({ line: i + 4, text: alts[0], alts })) });
const fx = {
  pie: table(['Single', 'Double'], ['1 ½ cups AP flour', '3 cups AP flour'], ['1 tsp Salt', '1 1/2 tsp Salt'], ['½ cup Cold butter', '1 cup Cold butter'], ['1 Tbsp Sugar', '2 Tbsp Sugar']),
  eggnog: table(['', ''], ['8 Eggs', '12 Eggs'], ['2 cups Cream', '3 cups Cream'], ['¾ cup Sugar', '1 1/8 cup Sugar']),
  hoisin: table(['', '', ''], ['2 Tbsp. Soy Sauce', '¼ cup Soy Sauce', '½ cup Soy Sauce'], ['1 ½ tsp. Molasses', '1 Tbsp. Molasses', '2 Tbsp. Molasses'], ['1 tsp. Vinegar', '2 tsp. Vinegar', '4 tsp. Vinegar']),
  quiche: table(['9” Quiche', '10” Quiche'], ['One pie shell', 'One pie shell'], ['1 cup Swiss cheese', '1 ½ cup Swiss cheese'], ['1/3 cup Onion', '½ cup Onion'], ['4 Eggs', '6 Eggs']),
  pizza: table(['SMALL', 'LARGE'], ['9 oz Water', '12 oz Water'], ['3 Tbsp Olive oil', '¼ cup Olive oil'], ['¾ tsp salt', '1 tsp salt']),
  honeycomb: table(['Small', 'Double Batch'], ['1 ½ cup Sugar', '2 cups Sugar'], ['1 ½ cup Syrup', '2 cups Syrup'], ['1 ½ Tablespoon vinegar', '2 Tablespoon vinegar']),
  fajita: table(['Small batch', 'Large batch'], ['1 Tbsp salt', '¼ cup salt'], ['2 Tbsp chili powder', '1/2 cup chili powder'], ['1 tsp cumin', '4 tsp cumin']),
  icecream: table(['VANILLA', 'CHOCOLATE'], ['3 egg yolks', '3 egg yolks'], ['½ cup Sugar', '1 cup Sugar'], ['1 cup Milk', '1 cup Milk'], ['¼ tsp Salt', '¼ tsp Salt'], ['', '2 oz. Unsweetened baking chocolate']),
  cocoa: table(['ALL SUGAR', '75% SUGAR', '66% SUGAR', '50% SUGAR'], ['2 cups cocoa powder', '2 cups cocoa powder', '2 cups cocoa powder', '2 cups cocoa powder'], ['4 ½ cups sugar', '3 ¼ cups sugar', '3 cups sugar', '2 cups sugar'], ['1/8 tsp Salt', '1/8 tsp Salt', '1/8 tsp Salt', '1/8 tsp Salt']),
};

describe('scale', () => {
  it('parses quantity forms', () => {
    expect(parseNum('1 1/2')).toBe(1.5);
    expect(parseNum('1½')).toBe(1.5);
    expect(parseNum('1 ½')).toBe(1.5);
    expect(parseNum('¾')).toBe(0.75);
    expect(parseNum('1⁄4')).toBe(0.25);
    expect(parseNum('2.5')).toBe(2.5);
  });
  it('formats kitchen fractions', () => {
    expect(formatNum(1.5)).toBe('1½');
    expect(formatNum(1 / 3)).toBe('⅓');
    expect(formatNum(2)).toBe('2');
    expect(formatNum(0.99)).toBe('1');
    expect(formatNum(1 / 16)).toBe('⅛'); // never 0
    expect(formatNum(0.01)).toBe('⅛');
  });
  it('never scales an amount to 0', () => {
    expect(scaleLine('⅛ tsp salt', 0.5)).toBe('⅛ tsp salt');
    expect(scaleLine('1/8 tsp Salt', 1 / 3)).toBe('⅛ tsp Salt');
    expect(scaleLine('⅛ tsp salt', 0.5, 'metric')).toBe('0.3 ml salt');
    expect(scaleLine('1 pinch salt', 0.1)).toBe('⅛ pinch salt'); // not a count: a fraction
  });
  it('rounds things you can\'t split to whole numbers', () => {
    expect(scaleLine('3 large eggs', 0.5)).toBe('2 large eggs');
    expect(scaleLine('1 egg', 1 / 3)).toBe('1 egg');
    expect(scaleLine('3 eggs', 1 / 3)).toBe('1 egg');
    expect(scaleLine('1 egg', 1.5)).toBe('2 eggs');
    expect(scaleLine('2 cloves garlic, minced', 0.5)).toBe('1 clove garlic, minced');
    expect(scaleLine('3 egg yolks', 0.5)).toBe('2 egg yolks');
    expect(scaleLine('1 (15 oz) can black beans', 1.5)).toBe('2 (15 oz) cans black beans');
    expect(scaleLine('1 bunch cilantro', 2)).toBe('2 bunches cilantro');
    expect(scaleLine('2-3 eggs', 0.5)).toBe('1–2 eggs');
    expect(scaleLine('**3** eggs', 0.5)).toBe('**2** eggs');
    expect(scaleLine('3 large eggs', 1)).toBe('3 large eggs'); // unscaled: as written
    expect(scaleLine('2 EGGS', 1.5)).toBe('3 EGGS');
    expect(scaleLine('2 garlic cloves, roasted', 0.5)).toBe('1 garlic clove, roasted');
    expect(scaleLine('1 garlic clove', 0.5)).toBe('1 garlic clove');
    // the recipe already uses part of one: fractions stay
    expect(scaleLine('**1⁄2** head red cabbage', 0.5)).toBe('**¼** head red cabbage');
    expect(scaleLine('½ package cream cheese (4 oz)', 1.5)).toBe('¾ package cream cheese (4 oz)');
    // real amounts keep their fractions
    expect(scaleLine('½ onion, diced', 1.5)).toBe('¾ onion, diced');
    expect(scaleLine('1 stick butter', 1.5)).toBe('1½ stick butter');
    expect(scaleLine('1 eggplant', 1.5)).toBe('1½ eggplant');
    expect(scaleLine('2 Tbsp canola oil', 0.5)).toBe('1 tbsp canola oil');
  });
  it('scales lines as written', () => {
    expect(scaleLine('1 1/4 cups sugar', 2)).toBe('2½ cups sugar');
    expect(scaleLine('½ cup sugar', 2)).toBe('1 cup sugar');
    expect(scaleLine('1-2 cups milk', 2)).toBe('2–4 cups milk');
    expect(scaleLine('1½ tsp salt', 1 / 3)).toBe('½ tsp salt');
    expect(scaleLine('1 (10.75 ounce) can soup', 2)).toBe('2 (10.75 oz) cans soup');
    expect(scaleLine('salt to taste', 2)).toBe('salt to taste');
    expect(scaleLine('3 large eggs', 1)).toBe('3 large eggs');
  });
  it('converts units', () => {
    expect(scaleLine('1 cup milk', 1, 'metric')).toBe('237 ml milk');
    expect(scaleLine('2 lb beef', 1, 'metric')).toBe('907 g beef');
    expect(scaleLine('1 Tbsp oil', 1, 'metric')).toBe('15 ml oil');
    expect(scaleLine('3 large eggs', 2, 'metric')).toBe('6 large eggs');
  });
  it('US: the recipe\'s own units, tidied when scaling makes them awkward', () => {
    expect(scaleLine('3 tsp salt', 1, 'us')).toBe('3 tsp salt');
    expect(scaleLine('1 Tablespoon honey', 1, 'us')).toBe('1 tbsp honey');
    expect(scaleLine('16 Tbsp butter', 1, 'us')).toBe('16 tbsp butter');
    expect(scaleLine('16 Tbsp butter', 0.5, 'us')).toBe('½ cup butter');
    expect(scaleLine('1 ½ tsp salt', 2, 'us')).toBe('1 tbsp salt');
    expect(scaleLine('1 Tbsp sugar', 0.5, 'us')).toBe('1½ tsp sugar');
    expect(scaleLine('1 cup flour', 2, 'us')).toBe('2 cups flour');
    expect(scaleLine('2 cups flour', 0.5, 'us')).toBe('1 cup flour');
    expect(scaleLine('12 oz cheese', 2, 'us')).toBe('1½ lb cheese');
    expect(scaleLine('1 qt broth', 2, 'us')).toBe('2 qt broth');
    expect(scaleLine('1-2 Tbsp water', 4, 'us')).toBe('¼–½ cup water');
  });
  it('reads yields', () => {
    const n = (v: unknown) => parseYield(v)?.n ?? null;
    expect(n('6 servings')).toBe(6);
    expect(n('12-15')).toBe(12);
    expect(n(8)).toBe(8);
    expect(n('about 3 ½ pounds')).toBe(3.5);
    expect(n('Makes 30 dumplings')).toBe(30);
    expect(n('to taste')).toBeNull();
    expect(parseYield('Makes 2 dozen')).toEqual({ n: 24, unit: '', dozen: true });
    expect(parseYield('About 2 dozen tamales')).toEqual({ n: 24, unit: 'tamales', dozen: true });
    expect(parseYield('(2 to 3 dozen)')?.n).toBe(24);
    expect(parseYield('3 DOZEN LARGE COOKIES')?.unit).toBe('LARGE COOKIES');
    expect(n('about five 9” cakes')).toBeNull(); // a second number (9”): no single count
    expect(n('Makes about five cakes')).toBe(5);
    expect(n('3 small, 2 large or 1 jumbo pizza')).toBeNull();
    expect(n('6 - 11oz bottles')).toBeNull();
    expect(n('2 loaves or 12 buns')).toBeNull();
    expect(parseYield('12-15 servings.')).toEqual({ n: 12, unit: 'servings', dozen: false });
  });
  it('labels yields', () => {
    expect(yieldLabel(parseYield('Makes 2 dozen')!, 1)).toBe('24 (2 dozen)');
    expect(yieldLabel(parseYield('2 dozen tamales')!, 1.5)).toBe('36 tamales (3 dozen)');
    expect(yieldLabel(parseYield(6)!, 1.5, 'servings')).toBe('9 servings');
  });
});

describe('alternative portions', () => {
  const m = (g: { items: any[]; portions: string[] }) => portionMultipliers(g.items, g.portions);
  it('recognises the multiplier from the amounts', () => {
    expect(m(fx.pie)).toEqual([1, 2]);
    expect(m(fx.eggnog)).toEqual([1, 1.5]);
    expect(m(fx.hoisin)).toEqual([1, 2, 4]);
    expect(m(fx.quiche)).toEqual([1, 1.5]);
    expect(m(fx.pizza)).toEqual([1, 4 / 3]);
    expect(m(fx.fajita)).toEqual([1, 4]);
    expect(m(fx.honeycomb)).toEqual([1, 4 / 3]); // its amounts, not the "Double Batch" label
  });
  it('tells variants from sizes', () => {
    expect(m(fx.icecream)).toEqual([null, null]);
    expect(m(fx.cocoa)).toEqual([null, null, null, null]);
    expect(portionMultipliers([], ['Single', 'Double batch'])).toEqual([1, 2]);
  });
  it('shows the family amounts at a column multiplier, scales otherwise', () => {
    expect(resolvePortion([1, 2], 0, 2)).toEqual({ col: 1, scale: 1 });
    expect(resolvePortion([1, 2], 1, 3)).toEqual({ col: 1, scale: 1.5 });
    expect(resolvePortion([null, null], 1, 2)).toEqual({ col: 1, scale: 2 });
    const pie = (portion: number, factor: number) => groupLines(fx.pie, portion, factor, 'us').map((l) => l.text);
    expect(pie(1, 2)[0]).toBe('3 cups AP flour');
    expect(pie(0, 2)[1]).toBe('1 1/2 tsp Salt'); // the Double column as written, not 2 × 1 tsp
    expect(pie(0, 3)[0]).toBe('4½ cups AP flour');
    const ice = (portion: number) => groupLines(fx.icecream, portion, 1, 'us').map((l) => l.text);
    expect(ice(1)).toContain('2 oz Unsweetened baking chocolate');
    expect(ice(0).some((t) => t.includes('chocolate'))).toBe(false);
  });
});

describe('boldQty', () => {
  it('bolds the leading amount and its unit, nothing else', () => {
    expect(boldQty('1 ½ cups flour')).toBe('**1 ½ cups** flour');
    expect(boldQty('2 tbsp. butter, softened')).toBe('**2 tbsp.** butter, softened');
    expect(boldQty('¼–½ cup water')).toBe('**¼–½ cup** water');
    expect(boldQty('3 large eggs')).toBe('**3** large eggs');
    expect(boldQty('**1 ½** tbsp sugar')).toBe('**1 ½** tbsp sugar');
    expect(boldQty('Salt to taste')).toBe('Salt to taste');
  });
});
