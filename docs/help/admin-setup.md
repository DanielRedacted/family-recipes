---
title: Setting up the Mac app
summary: Install Recipes on the Mac, finish the first-start setup, choose the recipes folder (or several cookbooks), and keep the app running and up to date.
audience: admin
order: 70
---

The Mac app is the home of the cookbook: it reads the recipes folder, and it serves the same app to every phone and tablet on the Wi-Fi.

## Install

Download `Recipes_X.Y.Z_universal.dmg` from the GitHub Releases page and drag Recipes into **Applications**. Run it from there: updates can't be installed into a copy running from the disk image or the Downloads folder.

The app isn't notarized by Apple, so the first launch needs one extra step: right-click Recipes in Applications → **Open** → **Open**. In Terminal, `xattr -cr /Applications/Recipes.app` does the same.

## First start

The first start opens **Set up Recipes**, one page with the basics. Everything on it can be changed later in Settings.

- **Recipes folder**: where the recipes live, `~/Recipes` by default. **Use a different folder…** picks another, which the app reads at once.
- **Recipes from GitHub**: connect a private repository to sync the folder. See [GitHub sync](#/settings/help/github-sync).
- **This Mac**: **Open Recipes at login**.
- **Phones, tablets and other computers**: sharing on the Wi-Fi (on by default), the QR code, and **Keep this Mac awake while sharing**. See [Home network](#/settings/help/home-network).
- **Family PIN**: optional, any length. Without one, other devices can only read and use the whiteboard. See [People and PINs](#/settings/help/people).

Tap **Done** to finish.

## Cookbooks

**Settings → Cookbooks** lists the recipe folders the app serves, each with **Show in Finder** and its own GitHub sync. Most families have one. Every recipe is a plain Markdown file, one subfolder per category ([Recipe file format](#/settings/help/file-format)), so you can also edit them in any text editor or Obsidian, and changes show up in the app at once.

**Open or create a cookbook…** adds another folder (the picker's **New Folder** makes an empty one). It opens at once, without a restart, and every device gets a switcher at the top of its folder column; each keeps its own choice. People, the PIN, the whiteboard and the meal history are shared; recipes, favorites, feedback, the trash and GitHub sync are each cookbook's own. A cookbook can't be inside another or hold one, can't hold the app's data folder, and needs a folder name of its own (its name is the folder's name). **Only on this Mac** hides a cookbook from every other device; **Remove** stops serving it and leaves the folder as it is. A cookbook whose folder is missing at start (moved, or on a drive that isn't connected) is listed as missing and never made again; **Choose its folder…** points it at the folder's new place and turns its GitHub sync off, so a different folder is never pushed by surprise. See [Cookbooks](#/settings/help/cookbooks).

The app's own data is kept apart from the recipes, in `~/Library/Application Support/family.recipes.desktop/`: `settings.json`, `users.json`, `activity.log`, `messages.json` (the whiteboard), `made.json` and the `tls/` certificates. GitHub sync never sees these. The AI key and the GitHub token are kept in the macOS Keychain.

## Desktop app settings

**Settings → Desktop app** has:

- **Open Recipes at login**. Started this way, Recipes opens in the menu bar only, with no window and no Dock icon, ready for phones and tablets.
- **Updates**: what to do with a new version, and **Check for updates**.

## The menu-bar item

Closing the window doesn't quit Recipes. It keeps running in the menu bar, so phones and tablets can still reach it. The menu-bar icon has **Open Recipes** (brings the window back) and **Quit Recipes**. Quit only when no one needs the recipes on other devices.

## Updates

A few seconds after the app opens, it looks for a new version on GitHub. When there is one, a banner says **Recipes X.Y.Z is available**, with **What's new**, **Later** and **Update and restart**. Updating asks about any unsaved edit, downloads the new version and restarts into it.

**Settings → App → Updates** chooses what happens when there is a new version:

- **Just tell me** (the default): as above.
- **Download it, then ask me to restart**: it is downloaded first, so the banner says **Recipes X.Y.Z is ready** and **Restart to update** is quick.
- **Install it automatically**: it is downloaded and installed in the background. Recipes restarts into it when its window is closed (it stays in the menu bar) and nothing is unsaved, or when you choose **Restart to update**. Quitting and opening Recipes again also starts the new version.

With either of the last two, Recipes also looks every 6 hours while it runs, for a Mac that stays in the menu bar.

Each update is a new ad-hoc signed build, so macOS may ask again for Keychain access (the AI key) and for incoming network connections. Choose **Always Allow** and **Allow**.
