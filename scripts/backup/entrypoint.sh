#!/bin/sh
# Renders the crontab from $BACKUP_CRON_SCHEDULE and runs crond in the
# foreground so the container has a long-running process to supervise.
set -eu

SCHEDULE="${BACKUP_CRON_SCHEDULE:-0 2 * * *}"

# The schedule is written verbatim into root's crontab, so anything after the
# five time fields would run as a command. Whoever controls this variable —
# .env, or a compromised orchestrator — could otherwise execute arbitrary
# commands as root in the one container holding both PGPASSWORD and S3 write
# credentials. Restrict it to the characters a cron expression can contain,
# and require exactly five fields.
case "$SCHEDULE" in
    *[!0-9*,/\ -]*)
        echo "FATAL: BACKUP_CRON_SCHEDULE contains disallowed characters: $SCHEDULE" >&2
        exit 1
        ;;
esac

FIELD_COUNT=$(echo "$SCHEDULE" | tr -s ' ' | tr ' ' '\n' | grep -c .)
if [ "$FIELD_COUNT" -ne 5 ]; then
    echo "FATAL: BACKUP_CRON_SCHEDULE must have exactly 5 fields, got $FIELD_COUNT: $SCHEDULE" >&2
    exit 1
fi

CRONTAB_DIR="/home/backupuser/crontabs"
echo "$SCHEDULE /usr/local/bin/backup.sh >> /proc/1/fd/1 2>> /proc/1/fd/2" > "${CRONTAB_DIR}/backupuser"

echo "pg-backup starting. Schedule: $SCHEDULE"

# Runs as backupuser (see Dockerfile); -c points crond at a directory that
# user owns instead of the default root-owned /etc/crontabs.
exec crond -f -l 2 -c "$CRONTAB_DIR"
