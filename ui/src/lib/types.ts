/** `alts`: in a table with alternative portions, this ingredient's line for each portion ("" = not used) */
export interface Line { line: number; text: string; alts?: string[] }
/** `portions`: the labels of a table's alternative portions; `table`: its first and last line */
export interface Group { name: string | null; items: Line[]; portions?: string[]; table?: [number, number] }
export interface Recipe { ingredients: Group[]; steps: Group[] }

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

export interface Session {
  role: 'reader' | 'writer' | 'admin';
  pin_set: boolean;
  ai: boolean;
  shell: boolean;
  version: string;
}
