# Dependency Audit

Audit date: 2026-04-28. Rule: last release ≥ 2025-10-28 (≤6mo), language-native, exact pin (lockfile committed).

## Backend (Rust 1.95 · edition 2024)

Pinned in `backend/Cargo.toml`. Run `cargo tree` for the full graph.

| Crate | Pinned | Notes |
|---|---|---|
| tokio | `=1.52.1` | LTS until 2027-03 |
| axum | `=0.8.9` | + `ws` + `multipart` |
| axum-extra | `=0.12.6` | `cookie-private` for stateless sessions |
| tower-http | `=0.6.8` | cors · trace · compression-br · limit · request-id · timeout |
| tower_governor | `=0.8.0` | per-IP rate limit |
| sea-orm + sea-orm-migration | `=1.1.20` | sqlx-postgres + rustls |
| serde · serde_json | `=1.0.228` · `=1.0.149` | |
| thiserror · anyhow | `=2.0.18` · `=1.0.102` | |
| validator | `=0.20.0` | derive |
| argon2 (RustCrypto) | `=0.5.3` | |
| password-hash · rand_core | `=0.6.1` · `=0.10.1` | |
| utoipa · utoipa-axum | `=5.4.0` · `=0.2.0` | OpenAPI 3.1 |
| object_store | `=0.13.2` | aws/S3 feature |
| tracing · tracing-subscriber | `=0.1.44` · `=0.3.23` | env-filter + json |
| opentelemetry · opentelemetry-otlp · _sdk · tracing-opentelemetry | `=0.31.0` · `=0.31.1` · `=0.31.0` · `=0.32.1` | grpc-tonic |
| axum-prometheus · metrics | `=0.10.0` · `=0.24.3` | |
| lettre | `=0.11.21` | tokio-rustls SMTP |
| uuid | `=1.23.1` | v4/v5/v7 |
| chrono · time | `=0.4.44` · `=0.3.47` | |
| reqwest | (pinned in Cargo.toml) | + `stream` for SSE bytes_stream |

### Rejected (stale)

| Crate | Reason | Replacement |
|---|---|---|
| rspc | last 2025-01 | utoipa + openapi-typescript |
| axum-login | last 2024-07 | tower-sessions direct / `PrivateCookieJar` |
| openidconnect | last 2024-07 | deferred — not in scope |
| diesel-async | last 2024-03 | sea-orm |
| rust-argon2 | borderline + non-idiomatic | `argon2` (RustCrypto) |

## Frontend (TS strict, bun)

`frontend/package.json` uses `"latest"` for the active Lynx + framework set so `bun install --frozen-lockfile` against `bun.lock` provides reproducibility while letting the team refresh by re-resolving on demand. Pinned semver on third-party libs.

| Package | Range | Role |
|---|---|---|
| @lynx-js/react · react-use · web-core · web-elements | latest | runtime |
| @lynx-js/rspeedy · react-rsbuild-plugin · qrcode-rsbuild-plugin | latest | build |
| @lynx-js/tailwind-preset | `^0.4.0` | preset |
| @lynx-js/types · @lynx-js/preact-devtools | latest | dev |
| react-router · react-router-dom | `^6.30` | routing |
| zustand · zod · @tanstack/react-query | latest | state · validation · query |
| openapi-fetch · openapi-typescript | latest | typed client |
| i18next · react-i18next | `^26.0.8` · `^17.0.4` | i18n |
| lucide-static · qrcode · @types/qrcode | `^1.11.0` · `^1.5.4` · `^1.5.6` | icons + QR |
| tailwindcss · postcss · autoprefixer | `^3.4` · `^8.5.12` · `^10.5.0` | CSS |
| typescript · @types/react · @rsbuild/plugin-type-check | latest | TS |
| @playwright/test · otpauth | latest | E2E + TOTP test helper |

## Services (containerized)

| Service | Image | Purpose |
|---|---|---|
| postgres | `postgres:17-alpine` | OLTP |
| minio | `minio/minio:RELEASE.latest` | S3-compatible |
| caddy | `caddy:2-alpine` | reverse proxy + auto-HTTPS |
| nats | `nats:2-alpine` | optional events |
| mailpit | `axllent/mailpit` | dev SMTP |

## Verification

```bash
just audit                  # cargo-audit + cargo-deny (4 gates)
cargo tree --duplicates     # surface accidental dupes
bun outdated                # surface stale frontend
```

## Deferred

OIDC/SSO · passkeys (webauthn-rs) · Sparkling (mobile-only, breaks web) · Keycloak/Authentik · Stripe/Lago billing.
