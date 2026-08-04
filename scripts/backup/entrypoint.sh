#!/bin/sh
# Renders the crontab from $BACKUP_CRON_SCHEDULE and runs crond in the
# foreground so the container has a long-running process to supervise.
set -eu

echo "${BACKUP_CRON_SCHEDULE:-0 2 * * *} /usr/local/bin/backup.sh >> /proc/1/fd/1 2>> /proc/1/fd/2" > /etc/crontabs/root

echo "pg-backup starting. Schedule: ${BACKUP_CRON_SCHEDULE:-0 2 * * *}"

exec crond -f -l 2
