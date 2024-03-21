#!/bin/bash
set -e
# Change directory.
cd $(dirname "${BASH_SOURCE[0]}")
# Export some environment variables.
source deploy/.env
# Run server and block.
exec ./sync_msteams_jira_comments