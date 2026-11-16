---
version: alpha
name: Recipes
description: >-
  A family cookbook kept as plain-text recipes: a warm, paper-like reading app for the Mac and for
  phones and tablets at home. Light theme tokens below (the default everywhere); dark theme
  counterparts end in -dark.
colors:
  primary: "#7a52cc"
  primary-ink: "#5e3ba8"
  primary-soft: "#ebe3fb"
  on-primary: "#ffffff"
  on-success: "#1b1a22"
  background: "#faf9fd"
  surface: "#ffffff"
  sunken: "#f1eef8"
  border: "#e4e0ee"
  text: "#1b1a22"
  muted: "#69657a"
  danger: "#dc2626"
  success: "#16a34a"
  card-red: "#ffd6d6"
  card-orange: "#ffe5cc"
  card-yellow: "#fff9cc"
  card-green: "#d6f5d6"
  card-blue: "#cce5ff"
  card-purple: "#ead6ff"
  card-brown: "#e8d5c4"
  card-grey: "#efedf5"
  primary-dark: "#b39cf2"
  primary-ink-dark: "#c9b8f7"
  primary-soft-dark: "#2d2442"
  on-primary-dark: "#141319"
  background-dark: "#141319"
  surface-dark: "#1e1c24"
  sunken-dark: "#19171f"
  border-dark: "#2e2b37"
  text-dark: "#f1eff6"
  muted-dark: "#9c98a8"
typography:
  display:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 2rem
    fontWeight: 600
    lineHeight: 1.25
  recipe-section:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 1.35rem
    fontWeight: 600
    lineHeight: 1.25
  recipe-group:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 1.1rem
    fontWeight: 600
    lineHeight: 1.25
  recipe-body:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 17px
    fontWeight: 400
    lineHeight: 1.6
  card-title:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 17px
    fontWeight: 600
    lineHeight: 1.3
  brand:
    fontFamily: 'ui-serif, "New York", "Iowan Old Style", Charter, Georgia, serif'
    fontSize: 19px
    fontWeight: 600
    lineHeight: 1.2
  body:
    fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: 15px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.4
  caption:
    fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.4
  micro:
    fontFamily: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: 11px
    fontWeight: 500
    lineHeight: 1.3
  code:
    fontFamily: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace'
    fontSize: 0.9em
    fontWeight: 400
    lineHeight: 1.5
rounded:
  xs: 4px
  sm: 6px
  md: 8px
  lg: 10px
  pill: 16px
spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 48px
  sidebar-width: 290px
  reading-width: 760px
  page-max-width: 1200px
  compact-breakpoint: 899px
  touch-target: 36px
  touch-target-coarse: 40px
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: 12px
    height: 36px
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: 12px
    height: 36px
  button-ghost:
    backgroundColor: transparent
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    height: 36px
  button-ghost-hover:
    backgroundColor: "{colors.sunken}"
  button-selected:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.text}"
  button-danger:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.danger}"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: 12px
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.lg}"
  folder-card:
    backgroundColor: "{colors.card-orange}"
    textColor: "{colors.text}"
    typography: "{typography.card-title}"
    rounded: "{rounded.lg}"
    padding: 16px
  folder-card-red:
    backgroundColor: "{colors.card-red}"
  folder-card-yellow:
    backgroundColor: "{colors.card-yellow}"
  folder-card-green:
    backgroundColor: "{colors.card-green}"
  folder-card-blue:
    backgroundColor: "{colors.card-blue}"
  folder-card-purple:
    backgroundColor: "{colors.card-purple}"
  folder-card-brown:
    backgroundColor: "{colors.card-brown}"
  folder-card-grey:
    backgroundColor: "{colors.card-grey}"
  chip:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    height: 32px
  chip-selected:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.primary-ink}"
  property-chip:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.text}"
    typography: "{typography.caption}"
    rounded: "{rounded.pill}"
  tag:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.primary-ink}"
    typography: "{typography.micro}"
    rounded: "{rounded.lg}"
  favorite-chip:
    backgroundColor: "{colors.card-yellow}"
    textColor: "{colors.text}"
    rounded: "{rounded.pill}"
  tree-row:
    backgroundColor: transparent
    textColor: "{colors.text}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    height: 32px
  tree-row-hover:
    backgroundColor: "{colors.sunken}"
  tree-row-active:
    backgroundColor: "{colors.primary-soft}"
    textColor: "{colors.text}"
  sidebar:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.text}"
    width: 290px
  menu:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
  menu-item:
    rounded: "{rounded.sm}"
    padding: 8px
  toast:
    backgroundColor: "{colors.text}"
    textColor: "{colors.background}"
    rounded: "{rounded.md}"
    padding: 12px
  toast-error:
    backgroundColor: "{colors.danger}"
    textColor: "#ffffff"
  toast-success:
    backgroundColor: "{colors.success}"
    textColor: "{colors.on-success}"
  banner:
    backgroundColor: "{colors.card-yellow}"
    textColor: "{colors.text}"
    rounded: "{rounded.lg}"
    padding: 16px
  link:
    textColor: "{colors.primary-ink}"
  reader:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    typography: "{typography.recipe-body}"
    width: 760px
  reader-dark:
    backgroundColor: "{colors.background-dark}"
    textColor: "{colors.text-dark}"
  surface-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.text-dark}"
  sidebar-dark:
    backgroundColor: "{colors.sunken-dark}"
    textColor: "{colors.muted-dark}"
  button-primary-dark:
    backgroundColor: "{colors.primary-dark}"
    textColor: "{colors.on-primary-dark}"
  chip-selected-dark:
    backgroundColor: "{colors.primary-soft-dark}"
    textColor: "{colors.primary-ink-dark}"
  divider-dark:
    backgroundColor: "{colors.border-dark}"
  divider:
    backgroundColor: "{colors.border}"
  caption-text:
    textColor: "{colors.muted}"
---

# Recipes design system

## Overview

Recipes preserves a mother's and grandmother's handwritten and typed recipes as plain-text files,
and the app should feel like that family recipe box: warm, quiet, personal, and easy to read at
the stove. The people using it are family members of every age, on a Mac in the kitchen and on
phones and tablets around the house, often with flour on their hands.

- **Lavender paper, one purple accent.** Off-white pages with a faint lavender tint, a slightly darker "sunken"
  tone for the navigation column and quiet panels, and purple for the few things that act or are selected.
- **Serif for the recipe, sans for the app.** Recipe text, titles and cards use the system serif
  (New York on Apple devices) so a recipe reads like a printed card; controls, labels and lists use
  the system sans.
- **Content first.** The recipe is the page. Chrome stays small and muted; formatting marks are
  never shown, even while editing (Apple Notes style); navigation follows Obsidian (a folder column,
  breadcrumbs, `[[links]]`, properties).
- **Native feel.** System fonts, light paper pages on every device (dark only when chosen), standard
  controls (`select`, checkboxes) wherever they work.

## Colors

The palette: a paper neutral range tinted very lightly lavender, a purple accent that complements it,
and eight pastel card colours the family picks for folders and recipes.

- **Purple (`primary` #7a52cc):** the one accent; white text on it is 5.4:1. Primary buttons, list bullets and numbers, focus
  rings, the selected outline, dashed underlines on recipe links.
- **Purple ink (`primary-ink` #5e3ba8):** purple dark enough for text: links, group headings inside a
  recipe, the selected portion chip. 7.5:1 on the page background.
- **Purple wash (`primary-soft` #ebe3fb):** selected rows and chips, tags, the rule under recipe
  section headings.
- **Paper neutrals, lightly lavender:** `background` #faf9fd (pages), `surface` #ffffff (cards, inputs, menus),
  `sunken` #f1eef8 (navigation column, table headers, quiet panels), `border` #e4e0ee (every 1px
  line).
- **Text:** `text` #1b1a22 (16.5:1) and `muted` #69657a (5.35:1 on the page, 4.9:1 on sunken) for
  secondary information: counts, captions, breadcrumbs, the ↗ after web links.
- **Status:** `danger` #dc2626 for destructive actions and error toasts, `success` #16a34a for
  saved toasts.
- **Card colours (`card-*`):** red, orange, yellow, green, blue, purple, brown, grey pastels behind
  folder and recipe cards. They are chosen by the family in the icon picker, never used for meaning,
  and always carry `text` on top. Yellow doubles as the background of notice banners and the
  bookmarked chip.
- **Dark theme:** each token has a `-dark` counterpart (near-black pages with a lavender tint,
  `primary-dark` #b39cf2, `primary-ink-dark` #c9b8f7, `primary-soft-dark` #2d2442; card pastels become deep muted
  tones). Light is the default everywhere, whatever the system setting; dark applies only when
  `data-theme="dark"` is set (`theme.js`, from the saved `theme`). On the light purple of dark mode, put the dark background colour, not white, as text (7.9:1).

## Typography

Two system families and nothing downloaded: the system serif for anything that is part of a recipe,
the system sans for the app around it, and the system monospace only for the Markdown view and code.

- **display** (serif 2rem/600): the page title (Recipes on the home page, a folder, a recipe's title).
  A recipe's author sits right under its title as a byline: sans 14px in `muted`, "By …".
- **recipe-section** (serif 1.35rem/600): "Ingredients", "Directions"; underlined with a 2px purple
  wash rule.
- **recipe-group** (serif 1.1rem/600, purple ink): groups inside a section ("Crust", "Filling").
- **recipe-body** (serif 17px/1.6): ingredient lines, steps and notes in the reader and the
  formatted editor; capped at 760px wide. In the desktop reader the text itself is 6.5in (624px)
  wide, centred under the 760px recipe bar and property row by equal side padding.
- **card-title** (serif 17px/600) and **brand** (serif 19px/600): card names and the app name in
  the navigation column.
- **body** (sans 15px/1.5): the default for the interface, tree rows, buttons and inputs.
- **label** (sans 14px/500): chips, menu items, settings rows. **caption** (13px) for secondary
  lines; **micro** (11–12px/500) for counts, tags and keyboard hints, always in `muted`.
- Numbers that line up (counts) use tabular figures. Fractions are written with glyphs where the
  recipe has them (½, ¼). In ingredient lists the leading amount and its unit are bold ("**1 ½ cups**
  flour"); the words are never changed.
- Print: recipe text at 12pt in black on white, all chrome hidden.

## Layout

- **App icon:** the book on a `primary-soft` tile, the cover in `primary` with a `primary-ink` spine (`ui/public/icon.svg` is the
  master; `ui/e2e/icons.mjs` renders the web icons, `tauri icon` the desktop ones). The menu-bar icon is a monochrome template.
- **Desktop app window:** no title text, and a transparent title bar on the `sunken` colour (#f1eef8).
  The page has no space along its top edge there (`html.shell`), so the navigation column and the
  tab strip run straight into the title bar. Scrollbars are hidden everywhere (scrolling still works). Nothing bounces at the end of a scroll, neither the window nor any column or panel (`overscroll-behavior: none` on everything).
- **Tabs (desktop, any screen with two columns):** a strip at the top of the main column, on the
  `sunken` colour like the title bar. Each tab shows its page's icon and name (14px icon, 13px sans,
  `muted`; the active one in `text`), a × on hover, and is up to 200px wide (at least 96px; the strip
  scrolls sideways when full), with a + at the end. The active tab is the page's `background` colour
  with an 8px top radius, so it runs into the page below. Everything that navigates a page (Back,
  breadcrumbs, the page's buttons) is in that page's toolbar row, inside the tab. Each tab keeps its
  page alive (scroll, scaler, cook mode, an unsaved edit) and its own Back steps. ⌘-click or middle-click
  a link or a row, or "Open in new tab" in a ⋯ menu, opens a page in a new tab; ⌘T a new tab, Ctrl+Tab
  the next one; middle-click or × closes one (asking first about unsaved changes); drag to reorder. The
  navigation column is shared: switching tabs only moves its highlighted row, and its folders open and
  fold only when a page is opened. The tabs are kept per device. Phones have no strip and one page.
- **Navigation column header:** the app name, and across from it a search icon button (⌘K).
- **Folder title:** the folder's name with its item count beside it as a plain muted number (sans, 0.55em),
  on the desktop title and in the phone's top row; no separate "N items" heading.
- **Folder list view:** one row per recipe (name and time; no yield or tags); the name stays on one line and ends in … when it doesn't fit.
- **Home page:** no page title; it opens on the Suggested row (the phone's top row still names it Recipes).
- **Desktop:** a two-column grid exactly one window tall: the navigation column (`sidebar-width`
  290px, `sunken` background) and the main column, each scrolling on its own. Recipe pages keep
  their text to `reading-width` 760px; folder pages use a card grid up to `page-max-width` 1200px
  (`repeat(auto-fill, minmax(220px, 1fr))`, 16px gaps, folder cards too).
- **Compact (≤ `compact-breakpoint` 899px):** one column. The navigation column becomes a drawer
  (86vw, at most 330px) over a dimmed scrim; there is no top bar. A fixed bottom bar holds Folders
  (menu icon), Home (book icon) and Search; Add is in each folder's toolbar and Settings at the foot of the drawer.
  Breadcrumbs are hidden except Back, which sits in the page's toolbar row (beside ⋯ on a recipe).
  Pages can't be zoomed (pinch, double tap or focusing a field). Cards
  shrink to `minmax(150px, 1fr)` with 12px gaps; page padding drops to 16px with room for the bottom bar. Respect
  safe-area insets.
- **Spacing scale:** 2, 4, 8, 12, 16, 24, 48px (`xxs`…`xxl`), plus 20 and 32 where a step is needed;
  every margin, padding and gap is a multiple of 4 (2px only as a nudge inside small controls). Page
  padding 24px at the sides on desktop, the same on every page. The toolbar row (Back, breadcrumbs,
  buttons) is a full-width band with 8px above and below its controls; it stays fixed while the page
  below it scrolls. On desktop it is part of its tab's page (the `background` colour, under the active
  tab); on phones it is on the `sunken` colour, like the navigation column.
- **Touch:** controls are at least 36px tall, 40px where the pointer is coarse (rows, settings
  items), 52px for bottom-bar buttons.

## Elevation & Depth

The design is nearly flat: hierarchy comes from the paper tones and 1px `border` lines, not from
shadows.

- **Shadow (0 1px 3px, 8% black):** cards and the small ⋯ button on cards.
- **Raised shadow (0 4px 12px, 12% black):** things that float over the page: menus, toasts, the
  mobile drawer, dialogs.
- **Scrims:** 30% black behind the mobile drawer, 25% behind full-page busy states.
- Selection and focus use colour, not depth: an purple wash for selected rows and chips, a 2px purple
  outline for keyboard focus.

## Shapes

Soft, small radii; nothing sharp, nothing fully round except where something is a pill.

- `xs` 4px: inline code, keyboard hints.
- `sm` 6px: tree rows, menu items, small icon buttons.
- `md` 8px: buttons, inputs, selects, toasts, menus.
- `lg` 10px: cards, images in recipes, banners, tags.
- `pill` 16px: chips (properties, portions, settings chips on phones) at about 32px tall.

Icons are 1.8px-stroke, round-capped line icons (Lucide paths, `ui/src/components/Icon.svelte`) at 16px in
rows and chips, 18px in toolbars and 22px in the phone bars. Interface icons are always line icons, never
emoji or text glyphs (＋, ✕, ›): file types and Bookmarks (bookmark icon) in the navigation column, folders in search,
the tree chevron, property chips. Recipe and folder icons are emoji the family picks, or a line icon when
they haven't picked one. AI actions are plain text labels, with no sparkle.

## Components

- **Buttons.** `button` is the default (surface, 1px border, purple border on hover). `button-primary`
  is reserved for the one main action on a screen (Save, Cook mode, Create). `button-ghost` for
  toolbar icons; `button-selected` for a toggled state; `button-danger` text for destructive menu
  actions. Icon-only buttons are 36px square and always have an accessible label.
- **Inputs and selects.** Surface fill, 1px border, `md` radius; focus replaces the border with a
  2px purple outline. Use the native `select` for short choices (text type, theme, units); inside a row of other
  controls, the borderless `plain-select` (the Random filters).
- **Cards.** Folder cards are pastel (`card-*`), show an icon, the name in serif and a bare item
  count in the top-right corner (it gives way to the ⋯ button on hover). Recipe cards are all one height:
  the picture (or the card's pastel with its 44px icon in the top-left corner, 14px × 16px in) fills
  the card, 186px tall everywhere (room for the icon above the fullest text), and the title in serif (two lines at most), one details line (the time, never the yield or batch size), one row of tags and a bookmark icon for a
  bookmarked recipe sit on it at the bottom. The suggested card on the home page stretches to the height of
  the Random controls beside it (whose last row has no hairline). On a pastel the text is plain and the tags a 60% `surface` wash;
  on a photo a dark fade carries white text.
- **Chips.** Property chips on the sunken tone ("Prep 20 min"); missing standard properties appear as
  dashed outline chips with "＋". Portion chips ("Single ×1", "Double ×2") use `chip` and
  `chip-selected`. Tags use the purple wash.
- **Navigation column.** Tree rows 32px, a 16px chevron column for folders (a line icon for PDFs, images
  and other files), the recipe's emoji, the name, a muted count; files sit one step (16px) in from their folder. The active row uses the purple wash.
  A Bookmarks group sits on top, with a 12px gap before the folders. Clicking a folder row shows
  the folder's page and opens or closes its list of files (a second click closes it and stays on the page).
- **Reader.** The recipe in `recipe-body`, sections with an purple-wash rule, bullets and step
  numbers in purple, checkboxes in purple. Recipe links have a dashed purple underline; links to web
  pages are solid with a small muted ↗ and open in the browser. A recipe card above the text holds
  the serving scaler, portion chips and the recipe's filled-in property chips in one wrapping row,
  with an accent-ink "More ⌄" at its end. More opens a line with the source, the dashed add chips and
  the tags (and the original file); notes that aren't recipes show all their chips above the text. Cook mode (`button-primary`) sits in the toolbar row beside ⋯,
  and the unit choice (US / Metric) is in Settings → General, per device.
- **Formatted editor.** Looks like the reader (serif, sizes) with a slim toolbar; Markdown marks are
  never visible; tables are real grids with cell borders.
- **Feedback.** Toasts at the bottom, inverted (`text` on `background`), red with white text for
  errors, green with dark text when something was saved. Notices inside a page use the yellow `banner`.
- **Suggestion.** At the top of the home page, "Suggested": a recipe card (a Dinner recipe, shown at
  once) and beside it, in the same row and on the page itself (no card), the Random controls as a
  settings-style list: label left in `muted`, value right; Kind (default Dinner), Protein, Folder as
  plain menus, Bookmarked as a switch, hairlines between rows, then a `button-primary` "Random"; no
  padding, so it lines up with its grid column. After Random, the other matching recipes replace
  Bookmarks and Folders as a shuffled card grid under "N more matches", with a ghost "Show folders"
  button (tapping Home does the same).
- **Plain menus (`plain-select`).** A native `select` without the boxed field: no border or fill,
  the value in purple ink with the system's arrow. Used for the Random filters.
- **Cook mode.** A full-screen view with large checkable ingredient and step lists that keeps the
  screen awake.

## Do's and Don'ts

- Do keep a single purple accent; use it to mean "acts" or "selected", never decoration.
- Do set recipe text in the serif and interface text in the sans; don't mix them inside one control.
- Do use `primary-ink` (not `primary`) for purple text on light backgrounds.
- Primary buttons have white text on purple (`on-primary`, 5.4:1; `--on-accent` in the CSS). Don't put
  white text on green (3.2:1): the saved toast uses `on-success`, dark text at 5.2:1.
- Do show the recipe exactly as written: never change an amount or a word (bolding the leading
  amount and unit is the one restyle), never show Markdown marks, never hide a line.
- Don't use the card pastels to mean anything (status, category); they are the family's choice.
- Do make every control reachable at phone width and at least 36px (40px on touch) tall.
- Don't add shadows to show hierarchy; use the paper tones and 1px borders.
- Do follow the device's light or dark setting, and check both.
- Don't load web fonts or remote assets; the app must look the same offline.
