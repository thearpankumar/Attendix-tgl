#!/bin/sh
# Nightly pg_dump -> S3. Env vars: PGHOST/PGPORT/PGDATABASE/PGUSER/PGPASSWORD
# (standard libpq vars, picked up by pg_dump automatically), AWS_S3_BUCKET,
# AWS_REGION, AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, BACKUP_RETENTION_DAYS.
set -eu

TIMESTAMP=$(date -u +%Y%m%d_%H%M%S)
BACKUP_FILE="/backups/attendance_${TIMESTAMP}.dump"

log() {
    echo "[$(date -u +%FT%TZ)] $*"
}

log "Starting backup of ${PGDATABASE} on ${PGHOST}:${PGPORT:-5432}..."

pg_dump -h "$PGHOST" -p "${PGPORT:-5432}" -U "$PGUSER" -d "$PGDATABASE" -Fc -f "$BACKUP_FILE"

log "Dump complete: ${BACKUP_FILE} ($(du -h "$BACKUP_FILE" | cut -f1))"

aws s3 cp "$BACKUP_FILE" "s3://${AWS_S3_BUCKET}/postgres-backups/$(basename "$BACKUP_FILE")" --region "${AWS_REGION:-us-east-1}"

log "Uploaded to s3://${AWS_S3_BUCKET}/postgres-backups/$(basename "$BACKUP_FILE")"

# Local retention: the S3 object itself is retained/expired separately via a
# bucket lifecycle rule (see DEPLOYMENT.md) — this just keeps the container's
# own scratch volume from filling up.
find /backups -name "attendance_*.dump" -mtime "+${BACKUP_RETENTION_DAYS:-30}" -delete

log "Backup finished."
