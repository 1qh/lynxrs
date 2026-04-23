# Single-command dev and ops. Install just: `brew install just`.

default:
  @just --list

# ────────── dev ──────────

infra-up:
  docker compose -f infra/docker-compose.yml up -d postgres minio mailpit nats

infra-down:
  docker compose -f infra/docker-compose.yml down

obs-up:
  docker compose -f infra/docker-compose.yml -f infra/docker-compose.observability.yml up -d

backend-dev:
  cd backend && cargo watch -x run

frontend-dev:
  cd frontend && bun run dev

# ────────── build ──────────

backend-build:
  cd backend && cargo build --release --locked

backend-image tag="dev":
  docker build -t simu-backend:{{tag}} -f backend/Dockerfile backend

frontend-build:
  cd frontend && bun run build

# ────────── test ──────────

backend-test:
  cd backend && cargo nextest run --all-features

e2e:
  cd frontend && bun run openapi:codegen && bunx playwright test

visual-update:
  cd frontend && bunx playwright test e2e/visual.spec.ts --update-snapshots

# ────────── quality ──────────

lint:
  cd backend && cargo clippy --all-targets --locked -- -D warnings
  cd backend && cargo fmt --all -- --check

audit:
  cd backend && cargo audit
  cd backend && cargo deny check --config deny.toml

secret-scan:
  gitleaks detect --source=. --no-banner --no-git --config=.gitleaks.toml

trivy:
  trivy image --severity HIGH,CRITICAL --no-progress simu-backend:dev

sbom out="docs/SBOM.spdx.json":
  docker save simu-backend:dev -o /tmp/simu-backend.tar
  syft /tmp/simu-backend.tar -o spdx-json={{out}}
  rm -f /tmp/simu-backend.tar

load:
  SIMU_BASE=http://127.0.0.1:8088 k6 run ops/load.k6.js

# ────────── all ──────────

ci:
  just lint
  just backend-test
  just frontend-build
  just e2e
  just audit
  just secret-scan
