# Playwright E2E status

40/42 specs passing locally against `infra/docker-compose.yml` + a backend
launched with the e2e env (see `.github/workflows/ci.yml` `e2e` job env block).

## Running locally

```sh
docker compose -f infra/docker-compose.yml up -d postgres minio nats mailpit
docker exec simu-minio mkdir -p /data/simu-uploads

# Terminal A: backend with e2e-friendly limits
cd backend
RATE_LIMIT_RPS=10000 RATE_LIMIT_BURST=10000 PER_USER_RPS=10000 \
  REQUIRE_ADMIN_MFA=0 BIND_ADDR=0.0.0.0:8088 \
  env $(cat .env | xargs) ./target/release/simu-backend

# Terminal B: frontend dev server (Lynx)
cd frontend && bun run dev

# Terminal C: tests
cd frontend && bunx playwright test
```

## Known failures

- **`per-user-rl.spec`** — spawns its own backend on :8089 with low
  `PER_USER_RPS` to assert 429 responses. Hard-coded paths to
  `/Users/o/simu/backend/.env`; only runs locally on macOS dev box.
- **`ws.spec`** — Playwright `BrowserContext.addCookies` round-tripping
  the `simu_session` value across the API context → browser context →
  `WebSocket` constructor doesn't carry the cookie reliably. Same flow
  with raw curl works (verified). Not worth blocking CI on.

The same WS broadcast invariant is covered by `ws_receives_file_created_broadcast`
in `tests/integration.rs` (Rust integration test, no browser involved).

## CI

The `Playwright E2E` job is `continue-on-error: true` while we shake out
flake. The other gates (Rust backend unit + integration, Lynx frontend
typecheck + build, Security scans) are required and currently green.
