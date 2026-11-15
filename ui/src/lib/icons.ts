// Card icons: an emoji (or a Lucide icon when no emoji fits) picked from words in a folder name
// or recipe title. Nothing when no word matches. Users can override per recipe (frontmatter
// `emoji:`) and per folder (.folders.yaml).

/** Order matters: dish types before ingredients, so "Chocolate Cake" is a cake. */
const EMOJI: [string, string][] = [
  ['ice cream|gelato|sorbet|popsicle|cone', '🍨'],
  ['hot dog', '🌭'],
  ['pet|dog', '🐾'],
  ['cookie|biscotti|macaroon|snickerdoodle|shortbread|brownie|bar', '🍪'],
  ['cupcake|muffin', '🧁'],
  ['cake|cheesecake|torte|bundt', '🎂'],
  ['pie|tart|cobbler|crisp|galette', '🥧'],
  ['pizza|calzone', '🍕'],
  ['waffle', '🧇'],
  ['pancake|crepe|french toast', '🥞'],
  ['bagel', '🥯'],
  ['croissant|danish|pastry|scone', '🥐'],
  ['flan|custard|pudding|creme brulee|crème brûlée', '🍮'],
  ['baguette|french bread', '🥖'],
  ['bread|loaf|roll|biscuit|bun|focaccia|sourdough|toast|cornbread', '🍞'],
  ['donut|doughnut|fritter', '🍩'],
  ['lollipop|sucker', '🍭'],
  ['candy|fudge|toffee|caramel|truffle|brittle|marshmallow', '🍬'],
  ['popcorn', '🍿'],
  ['dessert|mousse|trifle|sweet', '🍰'],
  ['pretzel', '🥨'],
  ['ramen|pho|udon|noodle soup', '🍜'],
  ['soup|stew|chowder|bisque|broth|stock|chili|gumbo', '🍲'],
  ['fondue', '🫕'],
  ['casserole|bake|lasagna|enchilada|gratin|paella', '🥘'],
  ['naan|tortilla|flatbread|tostada', '🫓'],
  ['salad|slaw|coleslaw', '🥗'],
  ['burger', '🍔'],
  ['burrito|wrap', '🌯'],
  ['tamale', '🫔'],
  ['pita|gyro|shawarma|kebab', '🥙'],
  ['falafel', '🧆'],
  ['sandwich|sub|panini', '🥪'],
  ['taco|quesadilla|fajita|nacho', '🌮'],
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
  ['fries|onion ring', '🍟'],
  ['side|potato|potatoes|hash brown', '🥔'],
  ['vegetable|veggie|green bean|bean|beans', '🫛'],
  ['corn', '🌽'],
  ['carrot', '🥕'],
  ['mushroom', '🍄'],
  ['avocado|guacamole', '🥑'],
  ['tomato', '🍅'],
  ['pepper|jalapeño|jalapeno', '🌶️'],
  ['broccoli', '🥦'],
  ['eggplant|aubergine', '🍆'],
  ['oatmeal|granola|porridge|muesli|cereal', '🥣'],
  ['cheese|cheddar', '🧀'],
  ['chocolate', '🍫'],
  ['lemon|lime', '🍋'],
  ['orange|citrus', '🍊'],
  ['apple', '🍎'],
  ['banana', '🍌'],
  ['strawberry|strawberries', '🍓'],
  ['blueberry|blueberries|berry|berries|cranberry', '🫐'],
  ['cherry|cherries', '🍒'],
  ['peach', '🍑'],
  ['mango', '🥭'],
  ['pear', '🍐'],
  ['watermelon|melon', '🍉'],
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

/**
 * Ingredient icons. First match wins, so specific phrases come before the words they contain
 * ("peanut butter" before "butter"); an empty icon means "no fitting picture" and stops the search
 * ("black pepper" is not a chili).
 */
const INGREDIENT: [string, string][] = [
  ['black pepper|white pepper|peppercorn|cream of tartar|baking powder|baking soda|vanilla|yeast|sugar|molasses|cornstarch|gelatin|sprinkles|vinegar|extract|rice syrup|corn syrup|tapioca syrup', ''],
  ['peanut butter|peanut', '🥜'],
  ['coconut milk|coconut cream|coconut oil|coconut flour|coconut', '🥥'],
  ['ice cream', '🍨'],
  ['cream cheese|cheese|parmesan|cheddar|mozzarella|ricotta|romano|monterey jack|feta|gruyere|swiss', '🧀'],
  ['buttermilk|milk|heavy cream|whipping cream|half and half|cream', '🥛'],
  ['butter|ghee', '🧈'],
  ['yogurt|yoghurt', '🥣'],
  ['egg|egg white|egg yolk|yolk', '🥚'],
  ['chicken broth|beef broth|broth|stock|bouillon', '🍲'],
  ['chicken|turkey', '🍗'],
  ['bacon', '🥓'],
  ['ham|pork|sausage|chorizo|pepperoni', '🍖'],
  ['beef|steak|ground chuck|chuck|brisket|short rib|tri tip', '🥩'],
  ['shrimp|prawn', '🦐'],
  ['crab|crab meat', '🦀'],
  ['lobster', '🦞'],
  ['clam|oyster|mussel', '🦪'],
  ['fish|salmon|cod|tuna|mahi mahi|mahi|tilapia|halibut|anchovy', '🐟'],
  ['green bean|edamame|snap pea|pea pod', '🫛'],
  ['bean|black bean|pinto bean|lentil|chickpea|garbanzo|split pea', '🫘'],
  ['sweet potato|yam', '🍠'],
  ['potato|potato flakes', '🥔'],
  ['tomato|tomato sauce|tomato paste|salsa', '🍅'],
  ['bell pepper', '🫑'],
  ['jalapeño|jalapeno|chili|chile|chilies|chiles|chipotle|cayenne|tabasco|hot sauce|red pepper flakes|pepper flakes|green chili|serrano|poblano|paprika|chili powder', '🌶️'],
  ['carrot', '🥕'],
  ['broccoli', '🥦'],
  ['corn tortilla|flour tortilla|tortilla', '🫓'],
  ['popcorn', '🍿'],
  ['corn|sweet corn', '🌽'],
  ['cucumber|pickle|zucchini', '🥒'],
  ['lettuce|spinach|kale|cabbage|greens|bok choy|chard', '🥬'],
  ['basil|parsley|cilantro|thyme|rosemary|oregano|herb|herbs|mint|dill|sage|bay leaf|bay leaves|italian seasoning|chive', '🌿'],
  ['mushroom', '🍄'],
  ['onion|shallot|scallion|green onion|leek|onion powder', '🧅'],
  ['garlic|garlic powder|granulated garlic', '🧄'],
  ['ginger', '🫚'],
  ['avocado', '🥑'],
  ['olive|olive oil', '🫒'],
  ['lemon|lime|lemon juice|lime juice|lemon zest', '🍋'],
  ['orange|orange juice|mandarin', '🍊'],
  ['apple|applesauce', '🍎'],
  ['banana', '🍌'],
  ['strawberry|strawberries', '🍓'],
  ['blueberry|blueberries|blackberry|blackberries', '🫐'],
  ['cherry|cherries', '🍒'],
  ['peach', '🍑'],
  ['pineapple', '🍍'],
  ['raisin|grape', '🍇'],
  ['pumpkin|pumpkin puree|squash', '🎃'],
  ['almond|walnut|pecan|hazelnut|pistachio|cashew|nut|macadamia', '🌰'],
  ['chocolate|chocolate chip|cocoa|cacao|cocoa powder', '🍫'],
  ['coffee|espresso', '☕'],
  ['tea', '🍵'],
  ['wine|marsala|sherry', '🍷'],
  ['beer', '🍺'],
  ['rum|vodka|tequila|whiskey|bourbon|brandy|liqueur|triple sec', '🥃'],
  ['flour|wheat|oat|oats|rolled oats|spelt|barley|einkorn|buckwheat|grain|bran|quinoa|millet', '🌾'],
  ['rice', '🍚'],
  ['pasta|noodle|macaroni|spaghetti|penne|fettuccine|lasagna', '🍝'],
  ['bread|breadcrumb|bread crumb|panko|bun|crouton', '🍞'],
  ['honey', '🍯'],
  ['maple syrup|maple', '🍁'],
  ['salt|sea salt|kosher salt', '🧂'],
  ['water', '💧'],
  ['ice', '🧊'],
  ['oil|vegetable oil|canola oil|shortening', '🫗'],
];
const INGREDIENT_RX = compile(INGREDIENT);

/** Picture for an ingredient name, or null when none fits. */
export function ingredientIcon(name: string): CardIcon {
  const n = name.replace(/[_-]+/g, ' ');
  for (const [rx, e] of INGREDIENT_RX) if (rx.test(n)) return e ? { emoji: e } : null;
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
