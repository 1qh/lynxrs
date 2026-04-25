# Disaster Recovery Runbook

## RPO / RTO

| Tier | RPO | RTO |
|------|-----|-----|
| Database | 24h (next housekeeping run + backup cadence) | 1h |
| Object storage | 0 (S3 versioning + cross-region replication once enabled) | 30m |
| Audit log | 0 for last 90d (in DB), then archived to S3 ndjson | 4h to rebuild verification chain from archive |

## Pre-incident

- `just admin-create` an emergency-only break-glass admin (kept in 1Password vault).
- Verify backups land: every nightly run of `just backup-restore-e2e` should report drift <1%.
- `cargo deny check` + `cargo audit` run via CI on every PR.
- Trivy + gitleaks scan the repo + container image on each merge to main.

## Incident playbooks

### 1. Database is gone

```bash
# 1. Spin a fresh Postgres
docker compose -f infra/docker-compose.yml up -d postgres

# 2. Locate the latest backup (S3, key prefix `backups/<timestamp>.sql.gz`)
mc ls local/simu-uploads/backups/ | tail -3

# 3. Restore
mc cat local/simu-uploads/backups/20260424T193200Z.sql.gz | gunzip > /tmp/restore.sql
docker exec simu-postgres psql -U simu -d simu -f /tmp/restore.sql

# 4. Sanity-check audit chain integrity
curl -sb cookies http://localhost:8088/api/admin/audit/verify | jq
```

### 2. Object storage corrupted / lost

- If S3 versioning is on (recommended), restore each prefix `u/{uid}/` from the
  prior version.
- If versioning is off: SHA256 in `file_objects.sha256` allows verification per
  file. Issue user-visible "your file is being recovered" notices via the
  `/me/audit` UI.

### 3. Audit chain tamper detected

`/admin/audit/verify` returns `ok: false, broken_at: <uuid>`.

```bash
# 1. Pull the archive from S3 cold storage
mc ls local/simu-uploads/audit-archive/ | tail -10
mc cp local/simu-uploads/audit-archive/$(date +%Y%m%dT)*.ndjson /tmp/

# 2. Replay the chain from genesis → broken_at, compare hash-by-hash
# (script TBD; conceptually: foreach line, recompute sha256(prev||canonical))
```

### 4. Region down

Currently single-region. Multi-region is **not implemented** — accepting RTO of
hours during regional outage. Roadmap item: enable S3 cross-region replication
+ Postgres logical replica + failover DNS.

## Communications

- Status page: roadmap (currently no public status surface).
- Customer email blast: `just admin-create` then run `INSERT INTO outbox …`
  (mailer batches via housekeeping).

## Post-incident

- Write a 5-whys in `docs/incidents/YYYY-MM-DD.md`.
- File a follow-up to remove whatever made the failure possible.
- Run `just backup-restore-e2e` to confirm recovery process actually works
  before declaring incident closed.
