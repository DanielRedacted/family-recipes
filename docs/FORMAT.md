# Recipe file format

Recipes are plain Markdown files in an ordinary folder (default `~/Recipes`). Any editor works —
TextEdit, VS Code, Obsidian. The app never rewrites a file's body on its own: it saves exactly what
you type (only when you press Save), and only changes the frontmatter block when you use the
Properties panel or the Icon & colour dialog.

```markdown
---
tags: [thanksgiving, pie]
yield: 8 servings
prep_time: 20 min
cook_time: 1 hr
source: https://example.com/pumpkin-pie
emoji: 🎃
color: orange
nutrition:
  serving_size: 1 slice
  calories: 320 kcal
  fat: 14 g
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

## Equipment

- 9-inch pie plate

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
- `[[Name]]` links to another note by file name (or by title). The note it points to lists
  “Mentioned in” links back. In the editor, typing `[[` suggests recipe names.
- `.folders.yaml` (at the top of the folder) holds folder icons and card colours:

  ```yaml
  Pies:
    emoji: 🥧
    color: orange
  ```

## Frontmatter (all optional)

| Key | Meaning |
|---|---|
| `tags` | list, e.g. `[pie, holiday]` |
| `yield`, `servings` | e.g. `8 servings`, `2 loaves`; the leading number drives the serving scaler |
| `prep_time`, `cook_time`, `total_time` | human text, e.g. `1 hr 20 min` |
| `source`, `author` | where the recipe came from |
| `original` | path of the kept original file (PDF, DOCX) |
| `needs_review` | `true` when the converter could not find clear sections |
| `emoji` | the card icon; `none` hides it. Without it the app picks one from the title or folder name, or shows none |
| `color` | card colour: `red`, `orange`, `yellow`, `green`, `blue`, `purple`, `brown` or `grey` |
| `nutrition` | map of values as written, e.g. `calories: 320 kcal`. Keys: `serving_size`, `calories`, `carbohydrates`, `protein`, `fat`, `saturated_fat`, `unsaturated_fat`, `polyunsaturated_fat`, `monounsaturated_fat`, `trans_fat`, `cholesterol`, `sodium`, `potassium`, `fiber`, `sugar`, `vitamin_a`, `vitamin_c`, `calcium`, `iron` |

A value lives in one place: the importer and the converter put times, yield and nutrition in the
frontmatter only, not also as a line in the body.

Unknown keys are kept as they are.

## How the app reads a recipe

- **Title:** the first `# ` heading before any `## ` heading, otherwise the file name.
- **Sections:** a `##` heading named Ingredients / Directions / Instructions / Method / Preparation /
  Procedure(s) / Steps (any case, optional trailing colon). A prefix becomes a group:
  `## CAKE INGREDIENTS` is the "CAKE" group.
- **Groups:** `###` headings, or a bold-only line, inside a section.
- **Items:** list items; if a section has no list, each line of text.
- **Tables:** under an Ingredients heading, each table row is an ingredient. A `qty | name` row
  reads “qty name”; other rows keep every cell, separated by “ · ” (side-by-side batch sizes are
  shown, not scaled).
- **No headings at all:** the first list whose items mostly start with a quantity is the
  ingredients, and further “**Label**” + list pairs right after it are more ingredient groups. A
  table counts too when it has an “Ingredients” column, is a plain `qty | name` table, or lists
  batch sizes side by side (one name cell, the rest amounts with kitchen units). Charts such as
  cooking times are not read as recipes. What follows is the steps.
- **Equipment:** an ordinary `## Equipment` section (the importer writes one when the source lists
  tools).
- **Scaling:** only the leading quantity of an ingredient line changes (`1 ½`, `1½`, `1/2`, `1-2`,
  `2.5`); the rest of the line stays as written.

HTML is allowed (it is sanitised when shown) but the app itself only writes standard Markdown.
