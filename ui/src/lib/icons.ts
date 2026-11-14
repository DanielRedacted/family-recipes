// Card icons: an emoji (or a Lucide icon when no emoji fits) picked from words in a folder name
// or recipe title. Nothing when no word matches. Users can override per recipe (frontmatter
// `emoji:`) and per folder (.folders.yaml).

/** Order matters: dish types before ingredients, so "Chocolate Cake" is a cake. */
const EMOJI: [string, string][] = [
  ['ice cream|gelato|sorbet|popsicle', '🍨'],
  ['hot dog', '🌭'],
  ['pet|dog', '🐾'],
  ['cookie|biscotti|macaroon|snickerdoodle|shortbread|brownie|bar', '🍪'],
  ['cupcake|muffin', '🧁'],
  ['cake|cheesecake|torte|bundt', '🎂'],
  ['pie|tart|cobbler|crisp|galette', '🥧'],
  ['pizza|calzone|flatbread', '🍕'],
  ['pancake|waffle|crepe|french toast', '🥞'],
  ['bagel', '🥯'],
  ['croissant|danish|pastry|scone', '🥐'],
  ['bread|loaf|roll|biscuit|bun|focaccia|sourdough|toast|cornbread', '🍞'],
  ['donut|doughnut|fritter', '🍩'],
  ['candy|fudge|toffee|caramel|truffle|brittle|marshmallow', '🍬'],
  ['popcorn', '🍿'],
  ['dessert|pudding|custard|mousse|trifle|sweet', '🍰'],
  ['soup|stew|chowder|bisque|broth|stock|chili|gumbo', '🍲'],
  ['casserole|bake|lasagna|enchilada|gratin|paella', '🥘'],
  ['salad|slaw|coleslaw', '🥗'],
  ['sandwich|burger|sub|wrap|panini', '🥪'],
  ['taco|burrito|quesadilla|fajita|nacho', '🌮'],
  ['pasta|spaghetti|noodle|macaroni|mac|fettuccine|penne|ravioli|lo mein', '🍝'],
  ['rice|risotto|pilaf|fried rice', '🍚'],
  ['sushi', '🍣'],
  ['curry', '🍛'],
  ['dumpling|wonton|potsticker|bao', '🥟'],
  ['egg|omelet|omelette|frittata|quiche|eggnog', '🥚'],
  ['chicken|turkey|poultry|wing|drumstick', '🍗'],
  ['bacon|ham|pork|sausage', '🥓'],
  ['meat|beef|steak|roast|rib|brisket|tri tip|meatball|meatloaf|lamb|bbq|barbecue|jerky', '🥩'],
  ['shrimp|prawn|seafood|scallop|crab|lobster', '🦐'],
  ['fish|salmon|tuna|cod|halibut|tilapia', '🐟'],
  ['drink|smoothie|shake|lemonade|punch|juice|cocktail|soda|milk', '🥤'],
  ['coffee|latte|mocha|espresso', '☕'],
  ['tea|chai', '🍵'],
  ['cocoa|hot chocolate', '☕'],
  ['sauce|dressing|condiment|seasoning|spice|rub|marinade|gravy|salsa|mayonnaise|ketchup|mustard|dip|jam|jelly|syrup', '🫙'],
  ['appetizer|snack|starter|hors', '🫒'],
  ['side|potato|potatoes|fries|hash brown', '🥔'],
  ['vegetable|veggie|green bean|bean|beans', '🫛'],
  ['corn', '🌽'],
  ['carrot', '🥕'],
  ['mushroom', '🍄'],
  ['avocado|guacamole', '🥑'],
  ['tomato', '🍅'],
  ['pepper|jalapeño|jalapeno', '🌶️'],
  ['broccoli', '🥦'],
  ['cheese|cheddar|fondue', '🧀'],
  ['chocolate', '🍫'],
  ['lemon|lime', '🍋'],
  ['orange|citrus', '🍊'],
  ['apple', '🍎'],
  ['banana', '🍌'],
  ['strawberry|strawberries', '🍓'],
  ['blueberry|blueberries|berry|berries|cranberry', '🫐'],
  ['cherry|cherries', '🍒'],
  ['peach', '🍑'],
  ['pineapple', '🍍'],
  ['coconut', '🥥'],
  ['pumpkin|squash', '🎃'],
  ['peanut|nut|almond|pecan|walnut', '🥜'],
  ['honey', '🍯'],
  ['reference|chart|guide|conversion|measurement', '📖'],
];

/** Lucide fallbacks for words with no fitting emoji. */
const LUCIDE: [string, string][] = [
  ['time|timing|temperature|temp', 'clock'],
  ['equipment|utensil|tool', 'utensils'],
];

const compile = (t: [string, string][]) => t.map(([words, v]) => [new RegExp(`\\b(${words})(e?s)?\\b`, 'i'), v] as const);
const EMOJI_RX = compile(EMOJI);
const LUCIDE_RX = compile(LUCIDE);

export type CardIcon = { emoji: string } | { icon: string } | null;

/** Best icon for a name, or null when nothing fits. */
export function guessIcon(name: string): CardIcon {
  const n = name.replace(/[_-]+/g, ' ');
  for (const [rx, e] of EMOJI_RX) if (rx.test(n)) return { emoji: e };
  for (const [rx, i] of LUCIDE_RX) if (rx.test(n)) return { icon: i };
  return null;
}

/** Up to `n` distinct emoji suggestions for a name (best first), padded with common food emoji. */
export function suggestEmoji(name: string, n = 24): string[] {
  const n2 = name.replace(/[_-]+/g, ' ');
  const hits = EMOJI_RX.filter(([rx]) => rx.test(n2)).map(([, e]) => e);
  return [...new Set([...hits, ...EMOJI.map(([, e]) => e)])].slice(0, n);
}

export const COLORS = ['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'brown', 'grey'] as const;
export type Color = (typeof COLORS)[number];
// the order v1 used, so existing cards keep their colours
const HASH_ORDER = ['orange', 'yellow', 'green', 'blue', 'purple', 'red', 'brown', 'grey'];

/** Pastel for a card: the chosen colour, else a stable one from the name. */
export function cardColor(name: string, chosen?: unknown): string {
  if (typeof chosen === 'string' && (COLORS as readonly string[]).includes(chosen)) return `var(--p-${chosen})`;
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return `var(--p-${HASH_ORDER[h % HASH_ORDER.length]})`;
}

/**
 * The icon to show: an explicit choice (`emoji:`; "none" hides it), else a guess from the
 * name, else the fallback (a recipe falls back to its folder's icon).
 */
export function iconOf(name: string, chosen: unknown, fallback: CardIcon = null): CardIcon {
  if (chosen === 'none') return null;
  if (typeof chosen === 'string' && chosen.trim()) return { emoji: chosen.trim() };
  return guessIcon(name) ?? fallback;
}
