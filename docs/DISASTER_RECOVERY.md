# Disaster Recovery

## RPO / RTO

| Tier | RPO | RTO | Notes |
|---|---|---|---|
| Postgres (incl. `conversations`, `messages`, `webhook_deliveries`) | 24h | 1h | next housekeeping cron + nightly backup |
| Object storage (MinIO/S3) | 0 | 30m | requires S3 versioning + CRR (post-spike) |
| Audit chain | 0 (last 90d in DB) | 4h | rebuild from S3 ndjson archive |

## Pre-incident

```bash
just admin-create               # break-glass admin → 1Password
just backup-restore-e2e         # nightly drill, drift <1%
just audit                      # cargo-audit + cargo-deny on every PR
```

Trivy + gitleaks scan repo + image on each merge.

## Restore flow

```mermaid
flowchart TD
  D{What's gone?} -->|Postgres| P[restore-pg]
  D -->|Object store| O[restore-s3]
  D -->|Audit chain tamper| A[replay-chain]
  D -->|Region| R[manual failover · TBD]

  P --> P1[compose up -d postgres]
  P1 --> P2[mc cat backups/&lt;ts&gt;.sql.gz | gunzip → psql]
  P2 --> P3[curl /api/admin/audit/verify]

  O --> O1{S3 versioning on?}
  O1 -->|yes| O2[restore prior version per u/&lt;uid&gt;/]
  O1 -->|no| O3[verify via file_objects.sha256<br/>flag affected files in /me/audit]

  A --> A1[mc cp audit-archive/&lt;ts&gt;.ndjson]
  A1 --> A2[replay genesis → broken_at<br/>compare sha256(prev || canonical)]
```

### Postgres restore

```bash
docker compose -f infra/docker-compose.yml up -d postgres
mc ls local/simu-uploads/backups/ | tail -3
mc cat local/simu-uploads/backups/<ts>.sql.gz | gunzip > /tmp/restore.sql
docker exec simu-postgres psql -U simu -d simu -f /tmp/restore.sql
curl -sb cookies http://localhost:8088/api/admin/audit/verify | jq
```

Restore covers all 37 migrations: users, files, file_versions, shares, tags, stars, comments, audit_log, organizations, webhooks + deliveries, conversations, messages.

### Audit chain replay

```bash
mc ls local/simu-uploads/audit-archive/ | tail -10
mc cp local/simu-uploads/audit-archive/<ts>.ndjson /tmp/
# foreach line: recompute sha256(prev || canonical_json), compare row_hash
```

### Region down

Single-region today. Multi-region is roadmap (S3 CRR + PG logical replica + DNS failover). Accept hours-RTO during regional outage.

## Communications

Status page: roadmap. Email blast: `just admin-create` + `INSERT INTO outbox …` (mailer batches via housekeeping).

## Post-incident

Write 5-whys at `docs/incidents/YYYY-MM-DD.md` · file follow-up to remove the cause · `just backup-restore-e2e` before declaring closed.
