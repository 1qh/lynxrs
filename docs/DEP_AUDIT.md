# Dependency Audit

Audit date: 2026-04-24
Rule: last release ≥ 2025-10-24 (≤ 6 months), language-native, exact pin.

## Backend (Rust)

| Crate | Pinned | Last Release | Native | Status |
|---|---|---|---|---|
| tokio | 1.51.1 | 2026-04-08 | ✅ | PASS (LTS until 2027-03) |
| axum | 0.8.9 | recent | ✅ | PASS |
| tower-http | 0.6.8 | recent | ✅ | PASS (verify exact date) |
| tower-sessions | 0.15.0 | 2026-02-01 | ✅ | PASS |
| sea-orm | 1.1.19 | 2026-era | ✅ | PASS (stable; 2.0 still RC as of audit) |
| serde | 1.0 (latest) | continuous | ✅ | PASS (dtolnay, always active) |
| serde_json | 1.0 (latest) | continuous | ✅ | PASS |
| thiserror | 2.0 (latest) | continuous | ✅ | PASS |
| validator | latest | verify | ✅ | VERIFY |
| argon2 (RustCrypto) | 0.5.3 | verify | ✅ | VERIFY — preferred over `rust-argon2` (July 2025 = borderline) |
| tracing | 0.1 (latest) | continuous | ✅ | PASS |
| tracing-subscriber | 0.3 (latest) | continuous | ✅ | PASS |
| utoipa | 5.x | verify | ✅ | VERIFY |
| utoipa-axum | latest | verify | ✅ | VERIFY |
| object_store | 0.13.2 | recent (Apache Arrow, active repo) | ✅ | PASS (verify exact date) |
| apalis | 0.7.4 / apalis-postgres 1.0.0-rc.7 | 2026-04 | ✅ | PASS |
| async-nats | 0.46.0 | recent | ✅ | PASS (verify exact date) |
| webauthn-rs | 0.5.4 | 2025-12 | ✅ | DEFERRED from spike (not in scope) |

### Rejected (FAIL rule)

| Crate | Last | Reason | Replacement |
|---|---|---|---|
| rspc | 2025-01-28 (15mo) | stale | utoipa + openapi-typescript |
| axum-login | 2024-07-20 (21mo) | stale | tower-sessions direct |
| openidconnect | 2024-07-06 (21mo) | stale | DEFERRED — not needed (email+password only) |
| diesel-async | 2024-03-20 (25mo) | stale | sea-orm |
| rust-argon2 | 2025-07-17 (9mo) | borderline + less idiomatic | `argon2` (RustCrypto) |

## Frontend (TypeScript)

| Package | Pinned | Last Release | Native | Status |
|---|---|---|---|---|
| @lynx-js/react | 0.119.0 | 2026-04-20 (4d) | ✅ | PASS |
| @lynx-js/rspeedy | 0.12.5 | 2026-04-23 (1d) | ✅ | PASS |
| @lynx-js/react-use | 0.2.0 | 2026-03-24 | ✅ | PASS |
| @tanstack/react-router | 1.168.23 | 2026-04-19 (5d) | ✅ | PASS |
| @tanstack/react-query | 5.99.2 | 2026-04-22 (2d) | ✅ | PASS |
| @tanstack/react-form | 1.29.0 | 2026-04-15 (9d) | ✅ | PASS |
| zod | 4.3.6 | ~2026-01 (3mo) | ✅ | PASS |
| zustand | 5.0.12 | ~2026-03 (1mo) | ✅ | PASS |
| openapi-fetch | 0.17.0 | ~2026-02 (2mo) | ✅ | PASS |
| openapi-typescript | 7.13.0 | ~2026-02 (2mo) | ✅ (dev) | PASS |
| @playwright/test | 1.59.1 | 2026-04-09 (15d) | ✅ (dev) | PASS |
| typescript | 5.x latest | continuous | ✅ (dev) | PASS |

### Dropped from stack

- `@lynx-js/tailwind-preset` — 7mo ago, FAILS; not needed since CSS Modules picked.
- `lynx-ui` — existence unverified as distinct npm package; defer, use bare Lynx primitives + CSS Modules for spike.

## Services (self-host daemons — containerized)

| Service | Image tag strategy | Purpose |
|---|---|---|
| postgres | `postgres:17-alpine` | OLTP |
| minio | `minio/minio:RELEASE.latest` | S3-compatible storage |
| caddy | `caddy:2-alpine` | reverse proxy + auto-HTTPS |
| nats | `nats:2-alpine` | events (optional) |

All images pinned to specific tags in `docker-compose.yml`.

## Unverified / VERIFY items

Run before first commit:
- `cargo search tower-http` → confirm 0.6.8 date ≥ 2025-10-24
- `cargo search object_store` → confirm 0.13.2 date
- `cargo search async-nats` → confirm 0.46.0 date
- `cargo search validator` → latest version + date
- `cargo search argon2` → confirm 0.5.3 date
- `cargo search utoipa` → latest 5.x version + date

If any FAIL, note in this doc and swap.

## Deferred from spike

- OIDC/SSO (openidconnect stale; not needed for email+pw)
- Passkeys (webauthn-rs, can add post-spike)
- Sparkling (TikTok app layer; mobile-only, breaks web target)
- Keycloak/Authentik (deferred to post-spike SSO work)
- Billing (Stripe external, Lago self-host post-spike)
