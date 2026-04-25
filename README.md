# simu — Lynx + Rust self-hosted SaaS spike

Green, reproducible foundation for a world-class SaaS.
Every dep is language-native, pinned exact, and verified fresh (≤6 months at audit time).

## Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                            Browser / Lynx                               │
│   ReactLynx (rspeedy) → openapi-fetch + Zustand → /events/ws            │
└──────────────┬──────────────────────────────────┬───────────────────────┘
               │ HTTPS (Caddy reverse-proxy)      │ WSS
               ▼                                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         simu-backend (Rust 1.95)                        │
│   Axum 0.8 router  ▸ utoipa OpenAPI ▸ tower-http (CORS, compression)    │
│    ▸ tower_governor (per-IP RL) ▸ axum-prometheus (/metrics)            │
│    ▸ TraceLayer + tracing-opentelemetry → OTLP                          │
│    ▸ csrf_enforce ▸ token_scope_enforce ▸ per_user_rate_limit           │
│    ▸ inject_request_id_into_errors ▸ security_headers (CSP)             │
│                                                                         │
│   auth (cookie + bearer + TOTP MFA) ─▶ audit hash chain (sha256, pg     │
│                                          advisory_xact_lock)            │
│   files (mod, meta, versions, shares, tags, stars, comments, bulk,      │
│          trash, presign, thumb)                                         │
│   webhooks (HMAC-SHA256 signed, retry+backoff dispatcher)               │
│   housekeeping cron: pw/email-token GC, trash purge, audit archive →    │
│                       S3 ndjson, webhook delivery purge                 │
│   mailer (lettre + rustls SMTP) → Mailpit/SES                           │
│                                                                         │
│   SeaORM 1.1 → Postgres (rolling alpine, 30+ migrations)                │
│   object_store 0.13 → MinIO/S3 (presigned PUT/GET, multipart upload)    │
└────────────────────────────────────────────────────────────────────────┘
        │                          │                          │
        ▼                          ▼                          ▼
   NATS (TBD)              Prometheus + 8 alerts        Jaeger / OTLP
```

**Operational invariants (test-enforced):**

- Audit chain: `prev_hash || canonical_json(row)` → `row_hash`. `pg_advisory_xact_lock(4242424242)` serializes writes; `audit_chain_holds_under_concurrent_writes` test fires 30 parallel records and verifies the chain.
- Soft-delete cascades: file delete → revoke shares.
- Quotas: per-user (`USER_QUOTA_BYTES`) + per-org (`ORG_QUOTA_BYTES`) on upload + version + move.
- Admin actions require `totp_enabled` (override via `REQUIRE_ADMIN_MFA=0` for dev).
- CSRF: `simu_csrf` cookie ↔ `X-CSRF-Token` header double-submit on mutating requests.
- Strict CSP on JSON responses (`default-src 'none'`); relaxed for `/docs` only.
- Audit retention exports to S3 (`audit-archive/<ts>.ndjson`) before DB purge — refuses to purge if upload fails.

## What's here

### Backend (Rust, 33 MB Alpine image)

- **Axum 0.8.9** HTTP, **SeaORM 1.1.19** async ORM → Postgres 17
- **Encrypted stateless cookies** via `axum-extra` `PrivateCookieJar` — no server session store
- **argon2** password hashing on `spawn_blocking` for throughput
- **Email + password** signup / login / logout / me
- **Password reset** flow — hashed tokens, 60-min TTL, Mailpit for dev SMTP
- **File upload**: multipart + base64-JSON fallback (Lynx lacks `FormData`/`Blob`)
- **S3-compatible storage** via `object_store` 0.13.2 → MinIO
- **WebSocket events** (`/events/ws`) via `axum::ws` + `tokio::broadcast` (`FileCreated` broadcasts to subscribed clients)
- **OpenAPI 3.1** via `utoipa` 5.3.1 + `utoipa-axum` — consumed by `openapi-typescript` → `openapi-fetch` in the Lynx app (typed end-to-end)
- **Rate limiting** per IP via `tower_governor` 0.8.0 (env-tunable; `/health`+`/metrics` bypass)
- **Prometheus `/metrics`** via `axum-prometheus` 0.10.0
- **Request-ID** propagation via `tower-http` `SetRequestId`+`PropagateRequestId`
- **Graceful shutdown** on SIGTERM/SIGINT (drain in-flight)
- **tracing** JSON logs via `tracing-subscriber` 0.3.23
- **validator** input validation, **thiserror** typed errors

### Frontend (Lynx, 96 KB web + 100 KB native bundle)

- **ReactLynx** `@lynx-js/react` 0.119.0, TS 5.9 strict (`noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`)
- **Rspeedy** 0.14.2 builds both `main.lynx.bundle` (native) and `main.web.bundle` (Wasm-less web)
- **Zustand** 5.0.12 auth store
- **Typed API client** from `openapi-typescript` 7.13.0 + `openapi-fetch` 0.17.0
- **Lynx hooks** from `@lynx-js/react-use` 0.2.0
- **CSS Modules** (Rspeedy default)
- **WebSocket subscription** → auto-refresh file list on server `FileCreated` broadcast

### Infra (docker compose)

- **postgres:17.2-alpine** · **minio** · **nats** · **mailpit** · **caddy:2.8**
- **Observability layer**: Prometheus 2.55, Grafana 11.4 (provisioned with Prom+Loki datasources + `simu · backend overview` dashboard), Loki 3.4, Vector 0.44 (Docker-log → Loki), Valkey 8, GlitchTip definition
- **CI layer**: Forgejo 10 + forgejo-runner (register with one-time token after boot)
- **Ports shifted** to avoid conflicts (see `docs/PORTS.md`)

### Mobile runtimes installed

- Android 36.1 (Android 16 Baklava) emulator + `LynxExplorer.apk` 3.7.0
- iOS 26.4 sim iPhone 17 Pro + `LynxExplorer.app` 3.7.0 (from `tooling/`)

### Tests (all green)

| Layer | Count | Notes |
|---|---|---|
| Rust property (proptest) | 4/4 | argon2 round-trip, mutation/empty rejection, base64 round-trip |
| Rust integration (testcontainers) | 3/3 | signup+login+upload+list+logout, duplicate signup 409, bad password 401 |
| Playwright E2E (Lynx shadow DOM) | 5/5 | debug, smoke, ws, visual-regression, password-reset |

### Security

- `cargo-audit` clean (4 documented waivers in `deny.toml`)
- `cargo-deny` all 4 gates green
- `gitleaks` — no leaks
- `trivy` image scan — 0 HIGH/CRITICAL
- `syft` SBOM at `docs/SBOM.spdx.json`

### Performance

- k6 50 VUs × 50 s against release container: **4112 req, 0 failures**, p95 531 ms, avg 97 ms
  (p95 dominated by argon2; now hashing on `spawn_blocking` + release profile)

## Layout

```
spike/
├─ backend/               Cargo.toml + src/{main,auth,events,files,mailer,state,error,config}.rs
│                         entity/*.rs  migration/*.rs  tests/{integration,property}.rs
│                         Dockerfile  deny.toml
├─ frontend/              Lynx app · src/{App,state,api,index}  e2e/*.spec.ts  web-host/
│                         playwright.config.ts  tsconfig.json  package.json
├─ infra/                 docker-compose.yml (+ .observability, .cicd layers)
│                         Caddyfile  prometheus.yml  vector.toml  grafana/provisioning/
├─ ops/                   load.k6.js
├─ docs/                  DEP_AUDIT.md  SPIKE_PLAN.md  PORTS.md  SBOM.spdx.json
├─ tooling/               LynxExplorer.apk  LynxExplorer.app
├─ .forgejo/workflows/    ci.yml
├─ .gitleaks.toml  .pre-commit-config.yaml  .gitignore
├─ Justfile               one-command dev/build/test/audit/CI
└─ README.md
```

## Quick start

```bash
# 1. Secrets
cd infra && cp .env.example .env
# (SESSION_SECRET auto-generated on spin; regenerate with `openssl rand -hex 64`)

# 2. Infra
docker compose up -d postgres minio mailpit
docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d   # Prom+Grafana+Loki+Vector+Valkey

# 3. Backend
cd ../backend
export $(grep -v '^#' .env | xargs)
cargo run    # binds 127.0.0.1:8088

# 4. Frontend (separate terminal)
cd frontend
bun install --frozen-lockfile
bun run dev  # Rspeedy on 3000, web preview at /__web_preview?casename=main.web.bundle

# 5. Grafana: http://localhost:3100  (admin / simu_dev_grafana)
#    Prometheus: http://localhost:9090
#    Mailpit: http://localhost:8125  (SMTP port 1025)
#    MinIO console: http://localhost:9101  (simuadmin / simu_dev_minio)
```

## Task runners

```bash
just infra-up         # postgres+minio+mailpit+nats
just obs-up           # + Prom+Grafana+Loki+Vector+Valkey
just backend-dev      # cargo watch -x run
just frontend-dev     # rspeedy dev
just e2e              # regen openapi types + playwright
just backend-test     # nextest (property + integration)
just audit            # cargo-audit + cargo-deny
just trivy            # scan latest image
just sbom             # generate SPDX SBOM
just ci               # everything
```

## Scopes deferred (see `docs/SPIKE_PLAN.md`)

- SSO/OIDC, MFA, WebAuthn, billing, i18n, push notif
- Multi-tenant RLS design
- OTel distributed traces (Tempo/Jaeger)
- Chaos / mutation / fuzz test suites

## Non-stackable externals

- Apple Dev Program ($99/yr) for iOS device/Store
- Google Play Console ($25) for Store
- Payment processor (no OSS payment rails exist)
- Deliverable SMTP (self-host works, big-inbox deliverability not guaranteed)
- Push notif (APNs/FCM are Apple/Google)
