# VOICE — timers, conversions and a kitchen voice assistant (planned, not built)

A Google-Home-style wake-word assistant for **cooking timers** and **measurement conversions**,
and the timer and conversion features in the app that it depends on. Only the reader's conversions
exist so far (see "The app owns conversions").

The decisions were grilled and confirmed on 2026-07-07 for the earlier `cookbook` project, which
was meant to run on a Raspberry Pi in the kitchen. This version maps them onto this app; the
decisions themselves are unchanged. Build order: timers and conversions in the app first (useful
without voice), the voice daemon after.

## Where it runs

This app's server runs on the Mac (`Recipes.app`, or `recipes-server` on Linux; see
`deploy/linux/README.md`). The voice daemon, `voiced`, needs a microphone and speaker in the
kitchen, so it runs either:

- **on a Pi in the kitchen** (the original plan), talking to the Mac over the home network; or
- **on the Mac**, if the Mac is in the kitchen.

External mic + speaker over USB/AUX. Far-field mic quality matters more than anything in software:
test against range-hood noise early.

## Architecture

Two processes, one owner of state:

```
┌──── Mac: Recipes (Rust) ────────┐         ┌──── kitchen: voiced (Python) ─┐
│ /api/timers   ◄──── REST ───────┼─────────┤ openWakeWord (stock phrase,   │
│ /api/events   ──── SSE ─────────┼────────►│   e.g. "hey jarvis")          │
│   (timer started / ended)       │         │ STT: faster-whisper base int8 │
│ /api/convert  ◄──── REST ───────┼─────────┤   (fallback: Vosk + grammar)  │
│ UI: countdown chips, "set       │         │ intent: deterministic grammar │
│   timer" on steps, cook mode    │         │ TTS: Piper                    │
└─────────────────────────────────┘         └───────────────────────────────┘
```

- **Separate Python daemon** (`voiced`): the good audio components (openWakeWord, faster-whisper,
  Piper) are Python-native. Its own service (systemd on a Pi, a launch agent on a Mac); either
  process can crash or be absent without affecting the other. The app stays Rust and Svelte.
- **The app owns timers.** An in-memory timer list in the server (create, list, cancel) under
  `/api/timers`, with "timer started" and "timer ended" events added to the existing `/api/events`
  stream (`server.rs` `events`, today used only for vault changes). The daemon is just voice I/O:
  it speaks an announcement when the event fires. The UI gets countdown chips (visible in cook
  mode, `CookMode.svelte`) and a "set timer" button on steps that name a time, with no extra work.
  Timers survive daemon restarts.
- **The app owns conversions.** One implementation shared by the reader's US / Metric menu and the
  daemon. Today the conversions live in the UI (`ui/src/lib/scale.ts`, with spellings from
  `units.ts`); `/api/convert` needs them on the server too, so either port the tables to Rust with
  tests that pin both sides to the same answers, or move conversion to the server and have the UI
  call it. First version: unit math within a dimension (volume, weight) plus °F/°C. Volume ↔ weight
  questions answer "that depends on the ingredient". A curated, bundled ingredient-density table
  (about 150 entries: flour 120 g a cup, butter 227 g a cup, …) is the planned upgrade; never an AI
  estimate. **Built for nutrition:** `crates/core/data/foods.json` holds USDA's grams per ml (`ml`)
  and per item (`g`) for about 200 ingredient keys, from SR Legacy via the reviewed
  `crates/core/data/foods-map.tsv`, extended on each Mac by optional USDA lookups (owner's choice,
  2026-11-22); `GET /api/foods` serves it. **Built in the reader (2026-11-26):** Metric mode weighs
  solids with that table and keeps liquids in ml (`solidGramsPerMl`), and converts °F to °C
  (`toCelsius`), both in `ui/src/lib/scale.ts`. `/api/convert` would still need them on the server.
- **Editing rights.** Creating and cancelling timers changes state, so a daemon on another device
  needs the same rights as any phone or tablet: the editing PIN (`auth.rs`). Its requests carry the
  `X-Recipes: 1` header like the UI's.

## Intent handling

**Deterministic grammar only; no AI in the voice path**, even though the app has optional AI.
Timers and conversions are closed grammars (numbers × units × a few verbs). Regex + number-word
parsing over the transcript; an unmatched utterance gets a spoken "I didn't catch that", never a
guess. An AI fallback was considered and rejected: seconds of latency, it fires exactly when the
cook is already annoyed, and it is nondeterministic for commands a grammar nails.

Target intents:
- `set a timer for 10 minutes` / `set a pasta timer for 8 minutes` (named timers)
- `how long left` / `how long on the pasta timer`
- `cancel the (pasta) timer`
- `how many ml in 2 tablespoons` / `what's 350 fahrenheit in celsius`

## Wake word

**openWakeWord with a stock pretrained phrase** (e.g. "hey jarvis") to start: no licensing, fully
local, works on day one. Custom-train a phrase of our own later through openWakeWord's
synthetic-data pipeline if the stock phrase grates. Porcupine rejected: closed source, and an
account and API key in an otherwise fully local stack.

## Defaults to benchmark on the hardware (not re-grilled)

- STT: faster-whisper `base` int8 (about 1–2 s for short commands on a Pi 5); if too slow or short
  of memory (for example next to a local AI model on the same machine), drop to Vosk with a
  constrained grammar (lighter, and grammar mode is very reliable for closed command sets).
- TTS: Piper (no real competitor on a Pi). Pick a voice; cache common phrases ("timer set",
  "didn't catch that") as pre-rendered WAVs.
- Audio plumbing: ALSA via `sounddevice` on a Pi; beware USB mic sample-rate quirks.
