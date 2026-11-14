#!/usr/bin/env bash
# Start a fresh Claude Code session that continues Trailboard from where it left off.
cd "$(dirname "$0")/.." && exec claude "$(cat docs/SESSION_PROMPT.md)"
