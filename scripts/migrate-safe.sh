#!/usr/bin/env bash
# migrate-safe.sh — Pre-migration backup wrapper for naic
#
# Creates a timestamped MariaDB backup before running SeaORM migrations.
# If the backup fails, the migration does NOT proceed.
#
# Usage: ./scripts/migrate-safe.sh
#
# Reads DATABASE_URL from environment or .env file.
# Format: mysql://user:password@host:port/database

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BACKUP_DIR="$PROJECT_ROOT/backups"

# --- Load DATABASE_URL from .env if not set in environment ---
if [ -z "${DATABASE_URL:-}" ]; then
    ENV_FILE="$PROJECT_ROOT/.env"
    if [ -f "$ENV_FILE" ]; then
        # Extract DATABASE_URL line, strip quotes
        DATABASE_URL=$(grep -E '^DATABASE_URL=' "$ENV_FILE" | head -1 | sed 's/^DATABASE_URL=//' | sed 's/^"//' | sed 's/"$//')
    fi
fi

if [ -z "${DATABASE_URL:-}" ]; then
    echo "ERROR: DATABASE_URL is not set and no .env file found."
    echo "Set DATABASE_URL in your environment or create a .env file."
    exit 1
fi

# --- Parse DATABASE_URL: mysql://user:password@host:port/database ---
# Remove the mysql:// prefix
URL_BODY="${DATABASE_URL#mysql://}"

# Extract user:password
USER_PASS="${URL_BODY%%@*}"
DB_USER="${USER_PASS%%:*}"
DB_PASS="${USER_PASS#*:}"

# Extract host:port/database
HOST_DB="${URL_BODY#*@}"
HOST_PORT="${HOST_DB%%/*}"
DB_NAME="${HOST_DB#*/}"

DB_HOST="${HOST_PORT%%:*}"
DB_PORT="${HOST_PORT#*:}"

# Default port if not specified
if [ "$DB_PORT" = "$DB_HOST" ]; then
    DB_PORT="3306"
fi

# --- Create backup directory ---
mkdir -p "$BACKUP_DIR"

# --- Generate timestamped backup filename ---
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_FILE="$BACKUP_DIR/naic-backup-${TIMESTAMP}.sql"

echo "=== naic Safe Migration ==="
echo "Database: $DB_NAME @ $DB_HOST:$DB_PORT"
echo "Backup target: $BACKUP_FILE"
echo ""

# --- Step 1: Create backup ---
echo "[1/2] Creating database backup..."
DUMP_ERR=$(mktemp)
if mysqldump \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --user="$DB_USER" \
    --password="$DB_PASS" \
    --single-transaction \
    --routines \
    --triggers \
    "$DB_NAME" > "$BACKUP_FILE" 2>"$DUMP_ERR"; then
    BACKUP_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
    echo "  Backup created successfully ($BACKUP_SIZE)"
    rm -f "$DUMP_ERR"
else
    echo "ERROR: Database backup failed. Migration will NOT proceed."
    if [ -s "$DUMP_ERR" ]; then
        echo "  mysqldump output:"
        sed 's/^/    /' "$DUMP_ERR"
    fi
    rm -f "$BACKUP_FILE" "$DUMP_ERR"
    exit 1
fi

# --- Step 2: Run migration ---
echo "[2/2] Running database migration..."
cd "$PROJECT_ROOT"
if cargo loco db migrate; then
    echo ""
    echo "=== Migration completed successfully ==="
    echo "Backup saved at: $BACKUP_FILE"
else
    echo ""
    echo "ERROR: Migration failed!"
    echo "  To restore from backup:"
    echo "  mysql -h $DB_HOST -P $DB_PORT -u $DB_USER -p $DB_NAME < $BACKUP_FILE"
    exit 1
fi
