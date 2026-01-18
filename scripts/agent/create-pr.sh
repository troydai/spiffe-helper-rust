#!/usr/bin/env bash
# Creates a GitHub Pull Request using the Gemini CLI in oneshot mode.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROMPT_FILE="${SCRIPT_DIR}/../../docs/prompts/pr-create.md"

if [[ ! -f "${PROMPT_FILE}" ]]; then
    echo "error: Prompt file not found at ${PROMPT_FILE}"
    exit 1
fi

echo "Starting Gemini CLI to create/update PR..."

cat ${PROMPT_FILE} | gemini --yolo --model gemini-3-flash-preview
