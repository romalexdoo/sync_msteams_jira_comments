#!/bin/bash
set -e
# Change directory to this script directory.
cd $(dirname "${BASH_SOURCE[0]}")
# Get updates from github
# git fetch
# git reset --hard origin/main
# Export some environment variables.
source deploy/.env
# Stop service if exists
# sudo systemctl stop sync_msteams_jira_comments.service
# Run backend.
exec cargo run --release --bin sync_msteams_jira_comments
