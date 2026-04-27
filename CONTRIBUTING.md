# Contributing

## Commits

`type(scope): summary` — types: `feat` `fix` `test` `chore` `docs` `refactor` `perf`. Scopes: `backend` `frontend` `infra` `ci` `ops` `docs`. Small, revertable. **Never** `Co-Authored-By` footers.

## Workflow

```mermaid
flowchart LR
  A[branch off main] --> B[code + test]
  B --> C[just lint]
  C --> D[just backend-test]
  D --> E[just e2e]
  E --> F[just audit]
  F --> G[commit]
  G --> H[PR · review · squash]
```

Pre-commit hook (`pre-commit install`) runs gitleaks + fmt + clippy + tsc.

## Hard rules

| Rule | Why |
|---|---|
| Bun only on TS/JS | no npm/pnpm/yarn/node — single toolchain |
| Latest-only | no polyfills, no BC shims, target newest OS/SDK |
| Deps ≤6 months | active, language-native, pinned `=x.y.z`, lockfile committed |
| `git init` step 0 | commit every milestone |
| Regression test per fix | every new endpoint → ≥1 E2E |
| `.env` gitignored | gitleaks gates CI + pre-commit; leak = rotate + rewrite |

Waivers go in `backend/deny.toml`. Verify release date on crates.io / npm before adding.

## Branching

`main` is protected. Feature branch → PR → squash.
