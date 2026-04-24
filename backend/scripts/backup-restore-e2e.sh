#!/usr/bin/env bash
# End-to-end backup+restore verification.
#
# Flow:
#   1. POST /admin/backup → triggers pg_dump → uploads to S3 (minio).
#   2. Fetch artifact via mc.
#   3. Spin a throwaway `simu_restore` DB, restore dump into it.
#   4. Diff table row counts: source vs restored. Exit non-zero on mismatch.
#
# Requires: backend running on $BASE, admin@simu.local with password below,
# minio on :9100, postgres container `simu-postgres` on the host docker.
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:8088}"
ADMIN_EMAIL="${ADMIN_EMAIL:-chain@simu.local}"
ADMIN_PW="${ADMIN_PW:-hunter2hunter2}"
jar="$(mktemp)"
trap 'rm -f "$jar" /tmp/simu-backup.sql.gz /tmp/simu-backup.sql' EXIT

# Login
curl -sS -c "$jar" -X POST "$BASE/api/auth/login" \
  -H "content-type: application/json" \
  -d "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PW\"}" >/dev/null
csrf=$(awk '/simu_csrf/{print $NF}' "$jar" | tail -1)
[[ -n "$csrf" ]] || { echo "login failed"; exit 1; }

# Trigger backup
resp=$(curl -sS -b "$jar" -H "x-csrf-token: $csrf" -X POST "$BASE/api/admin/backup")
key=$(echo "$resp" | python3 -c 'import json,sys;print(json.load(sys.stdin)["key"])')
echo "backup uploaded: $key"

# Fetch via mc (or fall back to direct minio url w/ admin creds)
export AWS_ACCESS_KEY_ID="${S3_ACCESS_KEY:-simuadmin}"
export AWS_SECRET_ACCESS_KEY="${S3_SECRET_KEY:-simu_dev_minio}"
bucket="${S3_BUCKET:-simu}"
endpoint="${S3_ENDPOINT:-http://127.0.0.1:9100}"

# Use awscli if available, else curl-signed via minio mc
if command -v aws >/dev/null 2>&1; then
  aws --endpoint-url "$endpoint" s3 cp "s3://$bucket/$key" /tmp/simu-backup.sql.gz
else
  # Minio supports anonymous with path-style if bucket has public download; we use mc.
  docker run --rm --network host -e MC_HOST_local="http://$AWS_ACCESS_KEY_ID:$AWS_SECRET_ACCESS_KEY@127.0.0.1:9100" \
    --entrypoint sh minio/mc -c "mc cat 'local/$bucket/$key'" > /tmp/simu-backup.sql.gz
fi

gunzip -f /tmp/simu-backup.sql.gz
echo "dump decompressed: $(wc -c < /tmp/simu-backup.sql) bytes"

# Counts from source
src_count=$(docker exec simu-postgres psql -U simu -d simu -tAc \
  "SELECT SUM(n_live_tup)::bigint FROM pg_stat_user_tables")
echo "source live rows: $src_count"

# Restore into throwaway db
docker exec simu-postgres psql -U simu -d simu -c "DROP DATABASE IF EXISTS simu_restore" >/dev/null
docker exec simu-postgres psql -U simu -d simu -c "CREATE DATABASE simu_restore" >/dev/null
docker cp /tmp/simu-backup.sql simu-postgres:/tmp/backup.sql >/dev/null
docker exec simu-postgres psql -U simu -d simu_restore -f /tmp/backup.sql >/dev/null 2>&1 || true
docker exec simu-postgres psql -U simu -d simu_restore -c "ANALYZE" >/dev/null

dst_count=$(docker exec simu-postgres psql -U simu -d simu_restore -tAc \
  "SELECT SUM(n_live_tup)::bigint FROM pg_stat_user_tables")
echo "restored live rows: $dst_count"

# Cleanup throwaway
docker exec simu-postgres psql -U simu -d simu -c "DROP DATABASE simu_restore" >/dev/null

# pg_stat_user_tables is approximate but stable post-ANALYZE. Allow 1% drift.
if [[ -z "$src_count" || -z "$dst_count" ]]; then
  echo "FAIL: row counts empty"; exit 1
fi
drift=$(( src_count > dst_count ? src_count - dst_count : dst_count - src_count ))
bound=$(( src_count / 100 + 1 ))
if (( drift > bound )); then
  echo "FAIL: row-count drift $drift > bound $bound (src=$src_count dst=$dst_count)"
  exit 1
fi
echo "OK: backup+restore row counts match (src=$src_count dst=$dst_count drift=$drift)"
