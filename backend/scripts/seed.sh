#!/usr/bin/env bash
# Populate the dev backend with demo data: 1 admin, 3 users, 1 org, 20 files.
# Idempotent-ish — re-run reuses emails, upload errors tolerated.
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:8088}"
ADMIN_PW="hunter2hunter2"
ADMIN_EMAIL="admin@simu.local"

here="$(cd "$(dirname "$0")/.." && pwd)"

# Admin user via CLI.
env $(cat "$here/.env" | xargs) \
  "$here/target/release/simu-admin" create \
  --email "$ADMIN_EMAIL" --password "$ADMIN_PW" 2>/dev/null || true

signup() {
  local email="$1" password="$2" jar="$3"
  curl -sS -c "$jar" -X POST "$BASE/api/auth/signup" \
    -H "content-type: application/json" \
    -d "{\"email\":\"$email\",\"password\":\"$password\"}" -o /dev/null || true
}

mutate() {
  local jar="$1" ; shift
  local csrf
  csrf=$(awk '/simu_csrf/{print $NF}' "$jar" | tail -1)
  curl -sS -b "$jar" -H "x-csrf-token: $csrf" -H "content-type: application/json" "$@"
}

for i in 1 2 3; do
  jar="/tmp/seed-u$i.cookies"
  rm -f "$jar"
  signup "user$i@simu.local" "hunter2hunter2" "$jar"
  for n in 1 2 3 4 5 6; do
    data=$(printf 'demo content %s %s' "$i" "$n" | base64)
    tag=$((n % 3 == 0 ? 0 : 1))  # keep varied
    mutate "$jar" -X POST "$BASE/api/files/json" \
      -d "{\"filename\":\"u${i}-file${n}.txt\",\"content_type\":\"text/plain\",\"data_base64\":\"$data\"}" >/dev/null
  done
done

# Org with owner user1 + members user2,user3
owner_jar="/tmp/seed-u1.cookies"
slug="demo-$(date +%s)"
org=$(mutate "$owner_jar" -X POST "$BASE/api/orgs" \
  -d "{\"name\":\"Demo Org\",\"slug\":\"$slug\"}")
org_id=$(echo "$org" | jq -r '.id // empty' 2>/dev/null || echo "")
if [[ -n "$org_id" ]]; then
  for email in user2@simu.local user3@simu.local; do
    mutate "$owner_jar" -X POST "$BASE/api/orgs/$org_id/members" \
      -d "{\"email\":\"$email\"}" >/dev/null || true
  done
fi

echo "seeded: admin=$ADMIN_EMAIL  users=user1..3@simu.local  pw=hunter2hunter2  org=$slug"
