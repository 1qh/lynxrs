# Host Port Map

Host already runs other stacks (noboil-postgres on 5432, etc). Simu shifts all exposed ports to avoid conflict.

| Service     | Host | Container | Purpose                |
|-------------|------|-----------|------------------------|
| Postgres    | 5533 | 5432      | DB (psql, clients)     |
| MinIO API   | 9100 | 9000      | S3 endpoint            |
| MinIO UI    | 9101 | 9001      | MinIO console          |
| NATS        | 4233 | 4222      | client                 |
| NATS monitor| 8233 | 8222      | health / stats         |
| Backend     | 8088 | 8080      | direct dev access      |
| Caddy HTTP  | 8081 | 80        | reverse proxy          |
| Caddy HTTPS | 8444 | 443       | reverse proxy + TLS    |

## URLs

- Frontend (via Caddy): https://localhost:8444
- Backend direct: http://localhost:8088
- Backend healthcheck: http://localhost:8088/health
- MinIO console: http://localhost:9101 (user: simuadmin)
- Postgres: `psql postgres://simu:simu_dev@localhost:5533/simu`
- NATS monitor: http://localhost:8233

## Other stacks on host (do not disturb)

- noboil-postgres on :5432
- Various dev servers 3002–3010, 4000–4002, 4500, 4600–4601
