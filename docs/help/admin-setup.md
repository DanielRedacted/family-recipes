---
title: Setting up the Mac app
summary: Install Recipes on the Mac, finish the first-start setup, choose the recipes folder, and keep the app running and up to date.
audience: admin
order: 70
---

The Mac app is the home of the cookbook: it reads the recipes folder, and it serves the same app to every phone and tablet on the Wi-Fi.

## Install

Download `Recipes_X.Y.Z_universal.dmg` from the GitHub Releases page and drag Recipes into **Applications**. Run it from there: updates can't be installed into a copy running from the disk image or the Downloads folder.

The app isn't notarized by Apple, so the first launch needs one extra step: right-click Recipes in Applications → **Open** → **Open**. In Terminal, `xattr -cr /Applications/Recipes.app` does the same.

## First start

The first start opens **Set up Recipes**, one page with the basics. Everything on it can be changed later in Settings.

- **Recipes folder**: where the recipes live, `~/Recipes` by default. **Use a different folder…** picks another; the button then reads **Done — restart Recipes**, because a new folder takes effect after a restart.
- **Recipes from GitHub**: connect a private repository to sync the folder. If you picked a different folder, connect it after the restart in **Settings → Recipes folder**. See [GitHub sync](#/settings/help/github-sync).
- **This Mac**: **Open Recipes at login**.
- **Phones, tablets and other computers**: sharing on the Wi-Fi (on by default), the QR code, and **Keep this Mac awake while sharing**. See [Home network](#/settings/help/home-network).
- **Family PIN**: optional, at least 6 characters. Without one, other devices can only read and use the whiteboard. See [People and PINs](#/settings/help/people).

Tap **Done** to finish.

## The recipes folder

**Settings → Recipes folder** shows the folder in use, with **Show in Finder** and **Use a different folder…**. After switching, restart Recipes to finish. Every recipe is a plain Markdown file, one subfolder per category ([Recipe file format](#/settings/help/file-format)), so you can also edit them in any text editor or Obsidian, and changes show up in the app at once.

The app's own data is kept apart from the recipes, in `~/Library/Application Support/family.recipes.desktop/`: `settings.json`, `users.json`, `activity.log`, `messages.json` (the whiteboard), `made.json` and the `tls/` certificates. GitHub sync never sees these. The AI key and the GitHub token are kept in the macOS Keychain.

## Desktop app settings

**Settings → Desktop app** has:

- **Open Recipes at login**. Started this way, Recipes opens in the menu bar only, with no window and no Dock icon, ready for phones and tablets.
- **Updates**, with **Check for updates**.

## The menu-bar item

Closing the window doesn't quit Recipes. It keeps running in the menu bar, so phones and tablets can still reach it. The menu-bar icon has **Open Recipes** (brings the window back) and **Quit Recipes**. Quit only when no one needs the recipes on other devices.

## Updates

A few seconds after the app opens, it looks for a new version on GitHub. When there is one, a banner says **Recipes X.Y.Z is available**, with **What's new**, **Later** and **Update and restart**. Updating asks about any unsaved edit, downloads the new version and restarts into it.

Each update is a new ad-hoc signed build, so macOS may ask again for Keychain access (the AI key) and for incoming network connections. Choose **Always Allow** and **Allow**.
