# Host Ports

Host already runs other stacks. simu shifts every exposed port to avoid collision.

| Service | Host | Container | URL |
|---|---|---|---|
| Postgres | 5533 | 5432 | `psql postgres://simu:simu_dev@localhost:5533/simu` |
| MinIO API | 9100 | 9000 | S3 endpoint |
| MinIO console | 9101 | 9001 | <http://localhost:9101> · `simuadmin` |
| NATS | 4233 | 4222 | client |
| NATS monitor | 8233 | 8222 | <http://localhost:8233> |
| Backend | 8088 | 8080 | <http://localhost:8088> · `/health` |
| Caddy HTTP | 8081 | 80 | reverse proxy |
| Caddy HTTPS | 8444 | 443 | <https://localhost:8444> (frontend) |
| Mailpit UI | 8125 | 8025 | <http://localhost:8125> · SMTP `:1025` |
| Grafana | 3100 | 3000 | `admin` / `simu_dev_grafana` |
| Loki | 3101 | 3100 | |
| Prometheus | 9090 | 9090 | |
| Ollama (host-installed, not in compose) | 11434 | — | `OPENAI_BASE_URL` default |

Do-not-disturb on this host: `noboil-postgres :5432` · dev servers `3002–3010` `4000–4002` `4500` `4600–4601`.
