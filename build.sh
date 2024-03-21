#!/bin/bash
set -e
# Change directory to this script directory.
cd $(dirname "${BASH_SOURCE[0]}")
# Get updates from github
git fetch
git reset --hard origin/main
# Build backend.
cargo build --release --bin sync_msteams_jira_comments
# create target directory
# sudo mkdir -p /opt/sync_msteams_jira_comments
# copy files to target directory
sudo systemctl stop sync_msteams_jira_comments.service
sudo cp deploy/sync_msteams_jira_comments.sh /opt/sync_msteams_jira_comments/
sudo cp target/release/sync_msteams_jira_comments /opt/sync_msteams_jira_comments/
sudo systemctl start sync_msteams_jira_comments.service
