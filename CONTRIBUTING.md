# Contributing

## Commits

Conventional-commit style: `type(scope): short summary`.

Types: `feat`, `fix`, `test`, `chore`, `docs`, `refactor`, `perf`.
Scopes we use: `backend`, `frontend`, `infra`, `ci`, `ops`, `docs`.

Keep commits small and focused — one feature or fix per commit. Rework freely;
prefer small revert-able steps over a single large commit.

## Before commit

```bash
just lint     # cargo fmt --check + cargo clippy -D warnings
just backend-test
just e2e
just audit    # cargo-audit + cargo-deny
```

Pre-commit hook (via `pre-commit install`) runs gitleaks + fmt + clippy + tsc automatically.

## Dependencies

Every new dependency must satisfy:

1. Actively maintained — last release within 6 months.
2. Native to its language — pure Rust or pure TS; no wrappers unless no native alternative.
3. Pinned exact version (`=x.y.z`) with lockfile committed.

Verify: release date on crates.io or npm before adding. Waivers go in `backend/deny.toml`.

## Tests

- Backend: `tests/property.rs` (proptest) + `tests/integration.rs` (testcontainers).
- Frontend: `e2e/*.spec.ts` (Playwright, hits live backend + Rspeedy preview).

Every bug fix ships with a regression test. Every new endpoint gets at least one E2E.

## Branching

`main` is protected. Work in feature branches → PR → review → squash.

## Secrets

`.env` files are gitignored; `.env.example` shows the shape.
`gitleaks` runs in CI + pre-commit. Any secret in history = rotate + rewrite.
