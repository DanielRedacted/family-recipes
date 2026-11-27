/** What an ingredient line asks for, read by the server for nutrition (`ingredient::Amount`). `qty`: the
 * amount as a number (a range's first); `key`: the normalised ingredient name; `grams`: a weight written
 * for the whole amount ("(150 g)"); `skip`: why it is left out ("optional", "for frying"…). */
export interface Amount { qty?: number; unit?: string; size?: string; key?: string; grams?: number; skip?: string }
/** `alts`: in a table with alternative portions, this ingredient's line for each portion ("" = not used);
 * `amounts`: one per portion (else one), null for a line that names no ingredient */
export interface Line { line: number; text: string; alts?: string[]; amounts?: (Amount | null)[] }
/** `portions`: the labels of a table's alternative portions; `table`: its first and last line */
export interface Group { name: string | null; items: Line[]; portions?: string[]; table?: [number, number] }
/** `sections`: one per top-level Ingredients heading when there are two or more: its name ("" = unnamed)
 * and the line it starts on (versions: `versionsOf` in scale.ts) */
export interface Recipe { ingredients: Group[]; steps: Group[]; sections?: { name: string; line: number }[] }

export interface Entry {
  path: string;
  kind: 'note' | 'pdf' | 'image' | 'other';
  size: number;
  mtime: number;
  etag: string;
  title: string;
  raw?: string;
  frontmatter?: Record<string, any>;
  frontmatter_error?: string;
  body_line: number;
  cover?: string;
  recipe?: Recipe;
  links?: string[];
  text?: string;
}

/** Someone who signs in (GET /api/users). What they contributed comes from the recipes (lib/people.ts). */
export interface Person {
  id: string;
  name: string;
  full_name: string;
  /** the fields below only reach signed-in devices */
  created?: number;
  favorites?: string[];
  /** an emoji, or a data: URL picture */
  picture?: string | null;
  /** names they were credited under before a rename */
  former?: string[];
}

export interface Session {
  role: 'reader' | 'writer' | 'admin';
  /** who this device is signed in as (the desktop app is "Admin"); null before signing in */
  user: { id: string; name: string; full_name: string } | null;
  pin_set: boolean;
  ai: boolean;
  /** the Mac's calendar events are shown (Settings → Calendar) */
  calendar: boolean;
  shell: boolean;
  version: string;
  /** first start: show the setup page */
  setup?: boolean;
  /** the cookbooks this device may open (Settings → Cookbooks), in order; named after their folders */
  books?: { id: string; name: string }[];
  /** the colour for every device (`#rrggbb`); null = the built-in purple */
  accent?: string | null;
  /** who uses the app (Settings → General → Group); the name is empty until the owner types one */
  group?: { kind: GroupKind; name: string };
}
export type GroupKind = 'family' | 'friends' | 'team' | 'other';
