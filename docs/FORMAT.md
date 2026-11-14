# Recipe file format

Recipes are plain Markdown files in an ordinary folder (default `~/Recipes`). Any editor works —
TextEdit, VS Code, Obsidian. The app never rewrites a file's body on its own: it saves exactly what
you type, and only changes the frontmatter block when you use the Properties panel.

```markdown
---
tags: [thanksgiving, pie]
yield: 8 servings
prep_time: 20 min
cook_time: 1 hr
source: https://example.com/pumpkin-pie
---

# Pumpkin Pie

![Pumpkin pie](/attachments/pumpkin-pie.jpg)

## Ingredients

### Crust

- 1 ¼ cups flour
- ½ cup cold butter

### Filling

- 2 large eggs
- 15 oz can pumpkin

## Directions

1. Heat oven to 425°.
2. Beat eggs slightly. Beat in remaining ingredients.

## Notes

Freezes well. See [[Pie Crust]].
```

## Folders and names

- The folder is the category (`Pies/Pumpkin Pie.md`). Move a file to recategorise it.
- `attachments/` holds pictures; `attachments/originals/` holds imported PDFs and Word files.
- `.trash/` holds deleted files and earlier versions of edited files until the trash is emptied.
- `[[Name]]` links to another note by file name (or by title).

## Frontmatter (all optional)

| Key | Meaning |
|---|---|
| `tags` | list, e.g. `[pie, holiday]` |
| `yield`, `servings` | e.g. `8 servings`, `2 loaves`; the leading number drives the serving scaler |
| `prep_time`, `cook_time`, `total_time` | human text, e.g. `1 hr 20 min` |
| `source`, `author` | where the recipe came from |
| `original` | path of the kept original file (PDF, DOCX) |
| `needs_review` | `true` when the converter could not find clear sections |

Unknown keys are kept as they are.

## How the app reads a recipe

- **Title:** the first `# ` heading before any `## ` heading, otherwise the file name.
- **Sections:** a `##` heading named Ingredients / Directions / Instructions / Method / Preparation /
  Procedure(s) / Steps (any case, optional trailing colon). A prefix becomes a group:
  `## CAKE INGREDIENTS` is the "CAKE" group.
- **Groups:** `###` headings, or a bold-only line, inside a section.
- **Items:** list items; if a section has no list, each line of text.
- **No headings at all:** the first list whose items mostly start with a quantity is the ingredients;
  what follows is the steps.
- **Scaling:** only the leading quantity of an ingredient line changes (`1 ½`, `1½`, `1/2`, `1-2`,
  `2.5`); the rest of the line stays as written.

HTML is allowed (it is sanitised when shown) but the app itself only writes standard Markdown.
