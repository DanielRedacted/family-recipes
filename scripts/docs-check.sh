#!/bin/sh
# Claude Code Stop hook (.claude/settings.json): keeps the code graph and the in-app Help current.
# - code edited since the graph was built -> `graphify update .` (AST only, free)
# - code edited since the Help pages (or the last "no docs affected" answer) -> ask the agent once
#   to update docs/help/ or say why none is affected; its next stop goes through and is remembered.
cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/..}" || exit 0
stamp=.claude/docs-checked

if grep -q '"stop_hook_active": *true' 2>/dev/null; then touch "$stamp"; exit 0; fi

# changed or new code files, from git (skips ignored build output)
code=$(git status --porcelain --untracked-files=all -- ui/src crates src-tauri/src docs/FORMAT.md | sed 's/^...//; s/.* -> //')
[ -n "$code" ] || exit 0

newer_than() { # files from $code modified after $1 (all of them if $1 is missing)
  [ -e "$1" ] || { echo "$code"; return; }
  echo "$code" | while IFS= read -r f; do [ "$f" -nt "$1" ] && echo "$f"; done
}

[ -n "$(newer_than graphify-out/graph.json)" ] && graphify update . >/dev/null 2>&1

newest_doc=$(ls -t docs/help/*.md "$stamp" 2>/dev/null | head -1)
pending=$(newer_than "${newest_doc:-/nonexistent}")
[ -n "$pending" ] || exit 0

files=$(echo "$pending" | head -8 | tr '\n' ' ')
jq -n --arg f "$files" '{decision: "block", reason: ("Code changed since the Help pages were last updated: " + $f + "— update the matching docs/help/ page (see AGENTS.md, Keeping docs and the graph current), or reply that no docs are affected and why.")}'
