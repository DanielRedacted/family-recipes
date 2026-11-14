export interface Line { line: number; text: string }
export interface Group { name: string | null; items: Line[] }
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
