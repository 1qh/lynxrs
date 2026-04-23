#!/usr/bin/env bash
# Comprehensive smoke: hit every endpoint, verify observability, confirm live services.
# Run:  SIMU_BASE=http://127.0.0.1:8088 ./ops/smoke-all.sh
set -euo pipefail

BASE="${SIMU_BASE:-http://127.0.0.1:8088}"
COOKIES="$(mktemp)"
trap 'rm -f "$COOKIES"' EXIT

pass() { printf "✅ %s\n" "$1"; }
fail() { printf "❌ %s\n" "$1"; exit 1; }

echo "=== Core endpoints ==="
curl -sf "$BASE/health" >/dev/null && pass "/health 200" || fail "/health"
curl -sf "$BASE/metrics" | grep -q "simu_http_requests_total" && pass "/metrics exposes prom counters" || fail "/metrics"
curl -sf "$BASE/api-docs/openapi.json" | grep -q '"openapi":"3.1.0"' && pass "/api-docs/openapi.json 3.1" || fail "/api-docs"

echo
echo "=== Auth flow ==="
email="smoke-$(date +%s)@example.com"
pw="hunter2hunter2"
curl -sf -X POST "$BASE/api/auth/signup" -H "content-type: application/json" -c "$COOKIES" \
  -d "{\"email\":\"$email\",\"password\":\"$pw\"}" >/dev/null && pass "signup 201"
curl -sf -b "$COOKIES" "$BASE/api/auth/me" >/dev/null && pass "me 200"

echo
echo "=== File upload (base64) ==="
body=$(python3 -c "import base64,json;print(json.dumps({'filename':'smoke.txt','content_type':'text/plain','data_base64':base64.b64encode(b'hello smoke').decode()}))")
FID=$(curl -sf -X POST "$BASE/api/files/json" -b "$COOKIES" -H "content-type: application/json" -d "$body" | python3 -c "import json,sys;print(json.load(sys.stdin)['id'])")
[ -n "$FID" ] && pass "upload_json 201 ($FID)" || fail "upload_json"
curl -sf -b "$COOKIES" "$BASE/api/files" | grep -q "$FID" && pass "list contains" || fail "list"
curl -sf -b "$COOKIES" "$BASE/api/files/$FID" >/dev/null && pass "download 200" || fail "download"
curl -sf -X DELETE -b "$COOKIES" "$BASE/api/files/$FID" >/dev/null && pass "delete 204" || fail "delete"

echo
echo "=== Password reset ==="
curl -sf -X POST "$BASE/api/auth/password/forgot" -H "content-type: application/json" \
  -d "{\"email\":\"$email\"}" >/dev/null && pass "forgot 202"
# Don't poll Mailpit here — that's covered by the e2e test.

echo
echo "=== Logout + 401 ==="
curl -sf -X POST -b "$COOKIES" -c "$COOKIES" "$BASE/api/auth/logout" >/dev/null && pass "logout 204"
code=$(curl -s -b "$COOKIES" -o /dev/null -w "%{http_code}" "$BASE/api/auth/me")
[ "$code" = "401" ] && pass "me-after-logout 401" || fail "me-after-logout got $code"

echo
echo "All smoke checks passed ✅"
