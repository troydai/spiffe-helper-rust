#!/bin/bash
# Script to post issue updates to GitHub
# Usage: GITHUB_TOKEN=your_token ./post-issue-updates.sh

set -e

if [ -z "$GITHUB_TOKEN" ]; then
    echo "Error: GITHUB_TOKEN environment variable not set"
    echo "Usage: GITHUB_TOKEN=your_token ./post-issue-updates.sh"
    exit 1
fi

REPO="troydai/spiffe-helper-rust"
API_URL="https://api.github.com/repos/$REPO/issues"

# Function to post comment to an issue
post_comment() {
    local issue_number=$1
    local file=$2

    echo "Posting update to issue #$issue_number..."

    # Read file content and escape for JSON
    local body=$(cat "$file" | jq -Rs .)

    curl -X POST \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        "$API_URL/$issue_number/comments" \
        -d "{\"body\": $body}" \
        > /dev/null 2>&1

    if [ $? -eq 0 ]; then
        echo "✓ Successfully updated issue #$issue_number"
    else
        echo "✗ Failed to update issue #$issue_number"
    fi
}

# Post all updates
post_comment 41 "issue-41-update.md"
post_comment 42 "issue-42-update.md"
post_comment 44 "issue-44-update.md"
post_comment 45 "issue-45-update.md"
post_comment 46 "issue-46-update.md"
post_comment 48 "issue-48-update.md"

echo ""
echo "All issue updates posted!"
