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
- `[[Name]]` links to another note by file name (or by title); a Markdown link to the file
  (`[mayonnaise](../Condiments/Mayonnaise.md)`) works too, also outside the app. The note it points
  to lists “Mentioned in” links back. In the editor, typing `[[` suggests recipe names.
- An ingredient named like another recipe links to it in the reader even without a link in the file
  ("pie crust" → Pie Crust). Only multi-word recipe names and Condiments recipes count, so "baking
  soda" doesn't link to Soda.
- `.folders.yaml` (at the top of the folder) holds folder icons and card colours:

  ```yaml
  Pies:
    emoji: 🥧
    color: orange
  ```

## Frontmatter (all optional)

| Key | Meaning |
|---|---|
| `tags` | list, e.g. `[pie, holiday]`; `Breakfast`, `Lunch`, `Dinner` feed the home page's suggestion |
| `yield`, `servings` | e.g. `8 servings`, `2 loaves`, `2 dozen`; the leading number (a dozen counts as 12) drives the serving scaler. A yield with a second number (`3 small, 2 large or 1 jumbo pizza`) is shown as written and the scaler shows ×N |
| `prep_time`, `cook_time`, `total_time`, `additional_time` | human text, e.g. `1 hr 20 min` |
| `source`, `author` | where the recipe came from |
| `original` | path of the kept original file (PDF, DOCX) |
| `needs_review` | `true` when the converter could not find clear sections |
| `emoji` | the card icon; `none` hides it. Without it the app picks one from the title or folder name, or shows none |
| `color` | card colour: `red`, `orange`, `yellow`, `green`, `blue`, `purple`, `brown` or `grey` |
| `favorite` | `true` puts the recipe under Favorites (home page and the top of the folder column) |
| `note` | a short remark kept out of the title, e.g. `not tried yet` |
| `aliases` | another name the recipe is known by (or a list); search and `[[links]]` find the recipe by it |
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
- **Tables:** under an Ingredients heading, each table row is an ingredient (see *Ingredient
  tables* below). The reading view and cook mode show a table as a scaled list.
- **Ingredient sets elsewhere:** every list, table or run of lines that mostly start with an
  amount is a set of ingredients, wherever it is: before the Ingredients heading (“**Dumplings**:”
  + list), with no headings at all (“**Pizza Sauce**” + list, “Add:” + list), or labelled under
  Directions (“### Turmeric Rice” + amounts). A bold or heading label names the set; an
  instruction label (“Add:”, “Mix:”, “Then add:”) stays a step. Numbered lists under Directions
  are steps. Charts such as cooking times are not read as recipes. With no Directions heading,
  the rest is the steps.
- **Lists of lists:** `- + 3 cups flour` (an item holding only a nested list) reads its nested
  items.
- **Equipment:** an ordinary `## Equipment` section (the importer writes one when the source lists
  tools).
- **Scaling:** only the leading quantity of an ingredient line changes (`1 ½`, `1½`, `1/2`, `1-2`,
  `2.5`); the rest of the line stays as written. Every set of ingredients scales by the same factor.
  Scaled amounts round to kitchen fractions and never below ⅛. Eggs, yolks, whites, cloves, cans,
  slices, pieces, heads, packages, bunches, sprigs and stalks round to a whole number, at least 1
  ("3 large eggs" × ½ → "2 large eggs"), and the word follows the number ("1 egg"), unless the
  recipe itself writes part of one ("½ head cabbage" keeps its fractions).

## Units

Every measurement unit after an amount is written one way, in lower case: `tsp`, `tbsp`, `cup` / `cups`
(cups when the amount is more than 1), `oz`, `fl oz`, `lb` (one or many), `pt`, `qt`, `gal`, `g`, `kg`,
`ml`, `l`. The reader shows the leading amount and unit of each ingredient in bold (the file is not
changed).
Counts stay words (`2 cloves`, `1 can`, `pinch`, `1 stick`). Temperatures with a scale are written
`350°F` / `175°C`; fractions have no suffix (`1/8`, not `1/8th`). The reader shows any other spelling
this way too, and the importer writes it this way; `scripts/normalize-units.ts <folder>` rewrites a
whole folder (each changed line is printed).

Scaling, **US** (the default): the recipe's own unit while the scaled amount reads well in it (⅛ up
to 3 tsp, 1 up to 4 tbsp, ¼ up to 16 cups, under 16 oz, ½ lb and up), otherwise the unit it does
read well in ("16 tbsp" × ½ → "½ cup"); qt, pt, fl oz and metric units keep their unit. **Metric**
converts to ml / g (l / kg from 950; one decimal under 1, e.g. "0.3 ml").

## Ingredient tables

The standard layout: one column per alternative portion (sizes like Single / Double, or versions
like Vanilla / Chocolate), the amounts first and the ingredient name last. The header names the
portions; it may be empty.

```markdown
| Single     | Double     | Ingredients         |
| ---------- | ---------- | ------------------- |
| 1 ½ cups   | 3 cups     | AP flour            |
| 1 tsp      | 1 1/2 tsp  | Salt                |
| -          | 1 tsp      | Vanilla             |
```

- An empty cell or `-` means that portion leaves the ingredient out; a row with no amounts (“One
  crust pie shell”) belongs to every portion. A cell may also hold a whole line (“¾ cup packed
  brown sugar”).
- **The multiplier** of each portion is worked out from its amounts compared with the first column
  (the median of the ratios, units converted where needed: 3 tbsp → ⅔ cup, 1 tbsp → ¼ cup… gives ×4; rounded to a kitchen fraction), or from the
  label (“Double”, “2x”) when the amounts can't tell. Columns whose amounts are about the same are
  versions, not sizes.
- **Reading view:** chips above the ingredients pick a portion (“Single ×1”, “Double ×2”); only that
  portion's amounts are shown. The −/+ scaler stops at each portion's multiplier and then shows the
  family's own amounts from that column; between them it scales the chosen column.
- The parser reads any column order (name first, name in the middle, whole lines side by side) and
  a header that holds the first row of amounts. A table with one amount column is better written as
  a list (`- ¾ cup Warm milk`).
- The formatted editor writes tables padded so the columns line up; opening a note only re-spaces
  them and doesn't count as a change.

**Links:** `[[Recipe name]]` and links to files in the recipes folder (`[text](../Pies/Pie Crust.md)`)
open in the app; links to web pages (`[text](https://…)`) are shown with ↗ and open in the browser.
In the reader a plain click follows a link; in the editor ⌘-click does.

HTML is allowed (it is sanitised when shown) but the app itself only writes standard Markdown.
