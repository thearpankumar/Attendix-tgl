# pg_dump matches the server's major version (postgres:18-alpine everywhere,
# see docker-compose.yml / docker-compose.postgres.yml), plus aws-cli for the
# S3 upload and busybox crond (already in alpine) for scheduling.
FROM postgres:18-alpine

RUN apk add --no-cache aws-cli && mkdir -p /backups

COPY backup.sh /usr/local/bin/backup.sh
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/backup.sh /usr/local/bin/entrypoint.sh

# Runs as a dedicated non-root user. crond needs a crontab directory it can
# write to; /etc/crontabs is root-owned by default, so entrypoint.sh points
# it at /home/backupuser/crontabs instead (see entrypoint.sh).
RUN addgroup -S backupuser && adduser -S -G backupuser backupuser \
    && mkdir -p /home/backupuser/crontabs \
    && chown -R backupuser:backupuser /backups /home/backupuser
USER backupuser

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
