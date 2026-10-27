#!/usr/bin/env bash
# Rebuild .claude/skills as per-skill symlinks into .agents/skills, so Claude Code
# discovers every project skill natively.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

rm -rf "$root/.claude/skills"
mkdir -p "$root/.claude/skills"
for dir in "$root"/.agents/skills/*/; do
  name="$(basename "$dir")"
  ln -s "../../.agents/skills/$name" "$root/.claude/skills/$name"
done
ls "$root/.claude/skills" | wc -l
