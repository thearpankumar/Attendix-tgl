#!/usr/bin/env bash
#
# Generates every secret the Attendix stack needs, straight into .env.
#
# Why this exists: setup.sh used to `cp .env.example .env` and then offer to
# start the stack under a menu item labelled "Production", which handed the
# service JWT_SECRET=CHANGE-THIS-TO-A-SECURE-RANDOM-STRING. The backend now
# refuses to boot on a placeholder or a short secret, so something has to
# produce real ones.
#
# Usage:
#   scripts/generate-secrets.sh              # fill in anything missing/placeholder
#   scripts/generate-secrets.sh --rotate     # regenerate ALL secrets (invalidates sessions)
#   scripts/generate-secrets.sh --check      # verify only; exit 1 if anything is unsafe
#   scripts/generate-secrets.sh --print      # print to stdout, touch no files
#   scripts/generate-secrets.sh --file path/to/.env
#
# Values are never echoed unless you ask for --print.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${SCRIPT_DIR}/.env"
EXAMPLE_FILE="${SCRIPT_DIR}/.env.example"

MODE="fill"      # fill | rotate | check | print
ASSUME_YES=0

#-------------------------------------------------------------------------------
# What we generate
#-------------------------------------------------------------------------------

# High-entropy tokens. These are HMAC keys and bearer tokens — nothing parses
# them, so the alphabet only needs to avoid shell/env quoting trouble.
TOKEN_SECRETS=(
    JWT_SECRET              # signs admin JWTs
    ADMIN_SECRET            # gates the public POST /api/admin/register
    METRICS_TOKEN           # Prometheus bearer token for /metrics
)

# Passwords that get interpolated into connection URLs (postgres://user:PASS@…,
# redis://:PASS@…). Same alphabet, which is deliberately URL-safe: a generated
# '@' or ':' or '/' would silently corrupt DATABASE_URL/REDIS_URL and the
# failure would look like a network problem, not a quoting bug.
PASSWORD_SECRETS=(
    POSTGRES_PASSWORD
    REDIS_PASSWORD
    GRAFANA_ADMIN_PASSWORD
    TIMESCALE_PASSWORD
)

TOKEN_LENGTH=48
PASSWORD_LENGTH=32

# Mirrors PLACEHOLDER_MARKERS in backend-rust/src/config/mod.rs. A secret
# containing any of these is rejected at boot, so generating one would be a
# confusing failure much later.
PLACEHOLDER_MARKERS=(
    change-this change_this changeme dev-secret dev-admin-secret
    your-secret placeholder test-key test-secret secure_password
)

# Must match MIN_SECRET_LEN in backend-rust/src/config/mod.rs.
MIN_SECRET_LEN=32

#-------------------------------------------------------------------------------
# Output helpers
#-------------------------------------------------------------------------------

if [ -t 1 ]; then
    C_RED=$'\033[0;31m'; C_GREEN=$'\033[0;32m'; C_YELLOW=$'\033[0;33m'
    C_BLUE=$'\033[0;34m'; C_RESET=$'\033[0m'
else
    C_RED=''; C_GREEN=''; C_YELLOW=''; C_BLUE=''; C_RESET=''
fi

info()    { printf '%s\n' "${C_BLUE}==>${C_RESET} $*"; }
ok()      { printf '%s\n' "${C_GREEN}  ok${C_RESET} $*"; }
warn()    { printf '%s\n' "${C_YELLOW}  !!${C_RESET} $*"; }
fail()    { printf '%s\n' "${C_RED} FAIL${C_RESET} $*" >&2; }

usage() {
    sed -n '3,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0
}

#-------------------------------------------------------------------------------
# Generation
#-------------------------------------------------------------------------------

# Emits `$1` characters from [A-Za-z0-9]. Base62 keeps the value safe to embed
# in a URL, a YAML scalar and a shell variable without any escaping.
random_string() {
    local length="$1" out=''
    if command -v openssl >/dev/null 2>&1; then
        # Over-generate: stripping non-base62 characters loses roughly 25% of
        # the bytes, so ask for well over what we need and trim.
        out="$(openssl rand -base64 $((length * 3)) | tr -dc 'A-Za-z0-9' | head -c "$length")"
    elif [ -r /dev/urandom ]; then
        out="$(LC_ALL=C tr -dc 'A-Za-z0-9' < /dev/urandom | head -c "$length")"
    else
        fail "No entropy source: install openssl or provide /dev/urandom."
        exit 1
    fi

    if [ "${#out}" -ne "$length" ]; then
        fail "Entropy source returned ${#out} characters, expected ${length}."
        exit 1
    fi
    printf '%s' "$out"
}

# Generates a secret guaranteed not to trip the backend's placeholder check.
# Base62 output cannot contain the hyphenated markers, but 'changeme' and
# 'placeholder' are theoretically reachable (~1 in 10^12). Re-rolling makes the
# guarantee exact rather than probabilistic, and costs nothing.
generate_secret() {
    local length="$1" candidate attempt=0
    while :; do
        candidate="$(random_string "$length")"
        if ! contains_placeholder "$candidate"; then
            printf '%s' "$candidate"
            return 0
        fi
        attempt=$((attempt + 1))
        if [ "$attempt" -ge 10 ]; then
            fail "Could not generate a clean secret after 10 attempts."
            exit 1
        fi
    done
}

contains_placeholder() {
    local value_lower marker
    value_lower="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
    for marker in "${PLACEHOLDER_MARKERS[@]}"; do
        case "$value_lower" in *"$marker"*) return 0 ;; esac
    done
    return 1
}

#-------------------------------------------------------------------------------
# .env manipulation
#-------------------------------------------------------------------------------

env_value() {
    # Prints the current value of $1 in $ENV_FILE, or empty if unset.
    [ -f "$ENV_FILE" ] || return 0
    sed -n "s/^$1=//p" "$ENV_FILE" | head -n1
}

set_env_value() {
    local key="$1" value="$2"
    if grep -q "^${key}=" "$ENV_FILE" 2>/dev/null; then
        # Base62 values contain no sed metacharacters and no '|', so this
        # substitution is safe without escaping.
        sed -i.tmp "s|^${key}=.*|${key}=${value}|" "$ENV_FILE"
        rm -f "${ENV_FILE}.tmp"
    else
        printf '%s=%s\n' "$key" "$value" >> "$ENV_FILE"
    fi
}

# `.env` carries REDIS_URL with the password inline, duplicating REDIS_PASSWORD.
# Compose builds its own copy from the parts, but the standalone value has to
# track the password or the two disagree and Redis auth fails at runtime.
sync_derived_urls() {
    local redis_password
    redis_password="$(env_value REDIS_PASSWORD)"
    [ -n "$redis_password" ] || return 0

    if grep -q '^REDIS_URL=' "$ENV_FILE" 2>/dev/null; then
        set_env_value REDIS_URL "redis://:${redis_password}@redis:6379"
        ok "REDIS_URL re-derived from REDIS_PASSWORD"
    fi
}

#-------------------------------------------------------------------------------
# Validation
#-------------------------------------------------------------------------------

# Returns 0 if the named secret is present, long enough and not a placeholder.
secret_is_healthy() {
    local key="$1" value
    value="$(env_value "$key")"

    [ -n "$value" ] || return 1
    [ "${#value}" -ge "$MIN_SECRET_LEN" ] || return 1
    contains_placeholder "$value" && return 1
    return 0
}

describe_problem() {
    local key="$1" value
    value="$(env_value "$key")"

    if [ -z "$value" ]; then
        printf 'missing'
    elif contains_placeholder "$value"; then
        printf 'placeholder'
    elif [ "${#value}" -lt "$MIN_SECRET_LEN" ]; then
        printf 'too short (%d < %d)' "${#value}" "$MIN_SECRET_LEN"
    else
        printf 'ok'
    fi
}

run_check() {
    local key problem unhealthy=0

    info "Checking secrets in ${ENV_FILE}"
    if [ ! -f "$ENV_FILE" ]; then
        fail ".env does not exist. Run this script without --check to create it."
        return 1
    fi

    for key in "${TOKEN_SECRETS[@]}" "${PASSWORD_SECRETS[@]}"; do
        problem="$(describe_problem "$key")"
        if [ "$problem" = "ok" ]; then
            ok "${key}"
        else
            fail "${key}: ${problem}"
            unhealthy=1
        fi
    done

    # Not a secret, but its absence means every compose file falls through to
    # its ':-postgres' default and the database runs as postgres/postgres.
    if [ -z "$(env_value POSTGRES_USER)" ]; then
        fail "POSTGRES_USER: missing (compose would default to 'postgres')"
        unhealthy=1
    else
        ok "POSTGRES_USER"
    fi

    if [ "$(env_value GRAFANA_ADMIN_USER)" = "admin" ]; then
        warn "GRAFANA_ADMIN_USER is still 'admin' — a named account is harder to guess"
    fi

    # Informational only — mobile/.env holds no secrets, so its absence never
    # makes the overall check unhealthy.
    if [ ! -f "${SCRIPT_DIR}/mobile/.env" ] && [ -f "${SCRIPT_DIR}/mobile/.env.example" ]; then
        warn "mobile/.env not set up yet — run this script (any mode) or 'cp mobile/.env.example mobile/.env'"
    fi

    if [ "$unhealthy" -ne 0 ]; then
        printf '\n'
        fail "One or more secrets are unsafe. Fix with: scripts/generate-secrets.sh"
        return 1
    fi

    printf '\n'
    ok "All secrets present and well-formed."
    return 0
}

#-------------------------------------------------------------------------------
# Modes
#-------------------------------------------------------------------------------

run_print() {
    local key
    info "Generated secrets (not written to any file):" >&2
    printf '\n'
    for key in "${TOKEN_SECRETS[@]}"; do
        printf '%s=%s\n' "$key" "$(generate_secret "$TOKEN_LENGTH")"
    done
    for key in "${PASSWORD_SECRETS[@]}"; do
        printf '%s=%s\n' "$key" "$(generate_secret "$PASSWORD_LENGTH")"
    done
}

# Sets ENV_FILE_PREEXISTED so we only back up a file that already had real
# content — backing up a .env we just created from the example would leave a
# stray file full of placeholders.
ENV_FILE_PREEXISTED=0

ensure_env_file() {
    if [ -f "$ENV_FILE" ]; then
        ENV_FILE_PREEXISTED=1
        return 0
    fi

    if [ ! -f "$EXAMPLE_FILE" ]; then
        fail "Neither ${ENV_FILE} nor ${EXAMPLE_FILE} exists."
        exit 1
    fi

    info "Creating ${ENV_FILE} from .env.example"
    cp "$EXAMPLE_FILE" "$ENV_FILE"
}

backup_env_file() {
    local stamp backup
    stamp="$(date -u +%Y%m%d%H%M%S)"
    backup="${ENV_FILE}.bak.${stamp}"
    cp "$ENV_FILE" "$backup"
    chmod 600 "$backup"
    warn "Previous .env backed up to $(basename "$backup") (contains old secrets — delete when done)"
}

# mobile/.env holds no secrets (just EXPO_PUBLIC_API_BASE_URL, and even that's
# optional — app.config.ts falls back to the production URL baked into
# app.json). Unlike the root .env, there's nothing here that needs a real
# value generated, so this is a plain copy-if-missing, run unconditionally
# (not gated by --rotate/--check the way TOKEN_SECRETS/PASSWORD_SECRETS are).
ensure_mobile_env() {
    local mobile_example="${SCRIPT_DIR}/mobile/.env.example"
    local mobile_env="${SCRIPT_DIR}/mobile/.env"

    [ -f "$mobile_example" ] || return 0

    if [ -f "$mobile_env" ]; then
        ok "mobile/.env already exists, leaving alone"
    else
        cp "$mobile_example" "$mobile_env"
        ok "mobile/.env created from mobile/.env.example (see it for iOS Simulator / Android Emulator / physical device URLs)"
    fi
}

run_generate() {
    local key length generated=0 skipped=0

    ensure_env_file

    if [ "$MODE" = "rotate" ] && [ "$ASSUME_YES" -ne 1 ]; then
        warn "--rotate regenerates EVERY secret."
        warn "This invalidates all admin sessions, and Postgres/Redis/Grafana will"
        warn "reject the old credentials until their containers are recreated:"
        warn "  docker compose down && docker compose up -d"
        printf '%s' "Continue? [y/N] "
        read -r reply
        case "$reply" in
            [yY]|[yY][eE][sS]) ;;
            *) info "Aborted."; exit 0 ;;
        esac
    fi

    if [ "$ENV_FILE_PREEXISTED" -eq 1 ]; then
        backup_env_file
    fi

    info "Generating secrets in ${ENV_FILE}"

    for key in "${TOKEN_SECRETS[@]}" "${PASSWORD_SECRETS[@]}"; do
        case " ${TOKEN_SECRETS[*]} " in
            *" $key "*) length="$TOKEN_LENGTH" ;;
            *)          length="$PASSWORD_LENGTH" ;;
        esac

        if [ "$MODE" = "fill" ] && secret_is_healthy "$key"; then
            ok "${key} already set, leaving alone"
            skipped=$((skipped + 1))
            continue
        fi

        set_env_value "$key" "$(generate_secret "$length")"
        ok "${key} generated (${length} chars)"
        generated=$((generated + 1))
    done

    # POSTGRES_USER is not a secret, but leaving it unset means the compose
    # default ('postgres') applies.
    if [ -z "$(env_value POSTGRES_USER)" ]; then
        set_env_value POSTGRES_USER "attendix"
        ok "POSTGRES_USER set to 'attendix'"
    fi
    if [ -z "$(env_value POSTGRES_DB)" ]; then
        set_env_value POSTGRES_DB "attendance_geotag"
        ok "POSTGRES_DB set to 'attendance_geotag'"
    fi

    # Same as POSTGRES_USER/POSTGRES_DB above — not secrets, but left unset
    # they'd fall through to compose defaults instead of the dedicated
    # telemetry database/user.
    if [ -z "$(env_value TIMESCALE_USER)" ]; then
        set_env_value TIMESCALE_USER "attendix_telemetry"
        ok "TIMESCALE_USER set to 'attendix_telemetry'"
    fi
    if [ -z "$(env_value TIMESCALE_DB)" ]; then
        set_env_value TIMESCALE_DB "attendix_telemetry"
        ok "TIMESCALE_DB set to 'attendix_telemetry'"
    fi

    sync_derived_urls

    # .env holds live credentials; it should not be world- or group-readable.
    chmod 600 "$ENV_FILE"
    ok "Permissions on .env set to 600"

    ensure_mobile_env

    printf '\n'
    info "${generated} generated, ${skipped} left alone."

    if [ "$(env_value GRAFANA_ADMIN_USER)" = "admin" ]; then
        warn "GRAFANA_ADMIN_USER is 'admin'. Consider a named account."
    fi

    printf '\n'
    info "Still needs a human — this script cannot invent them:"
    printf '  %s\n' \
        "AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY  (prefer an IAM role)" \
        "AWS_S3_BUCKET / AWS_REGION" \
        "DOMAIN, CORS_ORIGIN, WEBAUTHN_RP_ID, WEBAUTHN_ORIGIN" \
        "TRUSTED_PROXIES (the reverse proxy's subnet)"

    if [ "$MODE" = "rotate" ]; then
        printf '\n'
        warn "Recreate containers so the new credentials take effect:"
        warn "  docker compose down && docker compose up -d"
    fi
}

#-------------------------------------------------------------------------------

main() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --rotate|--force) MODE="rotate" ;;
            --check)          MODE="check" ;;
            --print|--stdout) MODE="print" ;;
            --yes|-y)         ASSUME_YES=1 ;;
            --file)           shift; ENV_FILE="${1:?--file needs a path}" ;;
            --file=*)         ENV_FILE="${1#--file=}" ;;
            -h|--help)        usage ;;
            *) fail "Unknown option: $1"; printf '\n'; usage ;;
        esac
        shift
    done

    case "$MODE" in
        check)  run_check ;;
        print)  run_print ;;
        *)      run_generate ;;
    esac
}

main "$@"
