---
title: People and PINs
summary: Who can sign in, what the family PIN allows, what only the Mac can do, and the record of who changed what.
audience: admin
order: 72
---

Every phone, tablet or other computer signs in as a person: a name, no password, plus the family PIN when one is set. People are there to record who did what. They don't have separate rights.

## Signing in

On a new device the app asks **Who's cooking?**. People tap their name, or add themselves with a short name (at most 40 characters) and a full name. The full name is written on recipes they add, write or edit. "Admin" is reserved for the Mac.

A device stays signed in for up to 400 days, and signing in again renews it. **Settings → General → Sign out** on the device ends it. Wrong PINs are rate-limited.

## The family PIN

Set it in **Settings → Phones & tablets → Family PIN** (or in the first-start setup). It needs at least 6 characters.

- **Set PIN** / **Change PIN**: every device has to sign in again, with the new PIN.
- **Remove PIN**: every device signs in again and can then only read.

## What each role may do

| | Signed in, no PIN set | Signed in with the PIN | The Mac's window |
|---|---|---|---|
| Read, search, cook mode, favorites | yes | yes | yes |
| Whiteboard, "Made today" | yes | yes | yes |
| Edit, import, bookmark, rename, move, delete, restore from Trash | no | yes | yes |
| AI features (when on) | no | yes | yes |
| Empty the Trash, Settings beyond General / Recent changes / Trash / Help, People, AI key, GitHub, Show in Finder, updates | no | no | yes |

Admin Help pages are only listed on the Mac.

## The built-in Admin

The Mac's own window never signs in. It is always the built-in **Admin**, with full rights. Its changes are credited to the person chosen in **Editing as** (in the folder column and in **Settings → People**). With nobody chosen they are recorded under "Admin", which has no full name for recipe credits.

## Settings → People

Only on the Mac. It lists everyone who has signed in or been added. Open a person to see:

- **Devices**: browser and device type, IP address and when each was last used (the latest 10 per person).
- **Changes**: their latest 200 changes, newest first, such as "Saved Soups/Minestrone.md", "Imported a recipe" or "Wrote on the whiteboard".
- **Rename…**, **Full name…** (or **Add full name…**) and **Remove**. Removing signs their devices out. What they changed stays, and they can sign in again as someone new.

**Add a person…** adds someone before they first sign in. **Changes made on this Mac** shows the log for the built-in Admin.

## The activity log

Every successful change is appended to `activity.log` in the data folder (`~/Library/Application Support/family.recipes.desktop/`), with the time, the person and the request. Signing in, AI requests and unsaved import steps aren't logged; moves and import saves don't name the recipe. People are kept in `users.json` beside it. Neither is in the recipes folder, so GitHub sync never uploads them.

See also: [Setting up the Mac app](#/settings/help/admin-setup) and [Home network](#/settings/help/home-network).
