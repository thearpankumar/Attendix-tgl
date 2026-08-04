# pg_dump matches the server's major version (postgres:16-alpine everywhere),
# plus aws-cli for the S3 upload and busybox crond (already in alpine) for
# scheduling.
FROM postgres:16-alpine

RUN apk add --no-cache aws-cli && mkdir -p /backups

COPY backup.sh /usr/local/bin/backup.sh
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/backup.sh /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
