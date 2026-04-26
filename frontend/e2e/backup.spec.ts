import { test, expect } from '@playwright/test'
import { newApi } from './_api'
import { execSync } from 'node:child_process'

test('admin /admin/backup runs pg_dump and returns key+size', async () => {
  const repoRoot = process.env.SIMU_REPO_ROOT ?? '/Users/o/simu'
  const adminEmail = `bk-${Date.now()}@t.local`
  const pw = 'hunter2hunter2'
  try {
    execSync(`test -f ${repoRoot}/backend/target/release/simu-admin`, { stdio: 'pipe' })
    execSync(
      `env $(cat ${repoRoot}/backend/.env | xargs) ${repoRoot}/backend/target/release/simu-admin create --email ${adminEmail} --password ${pw}`,
      { stdio: 'pipe' },
    )
  } catch (e) {
    // Skip silently if simu-admin can't run.
    test.skip(true, 'simu-admin CLI unavailable')
    return
  }
  // pg_dump must be on PATH for backup to succeed.
  try {
    execSync('command -v pg_dump', { stdio: 'pipe' })
  } catch {
    test.skip(true, 'pg_dump not installed')
    return
  }

  const admin = await newApi()
  const login = await admin.post('/api/auth/login', {
    data: { email: adminEmail, password: pw },
    headers: { 'content-type': 'application/json' },
  })
  expect(login.status()).toBe(200)

  const r = await admin.post('/api/admin/backup')
  expect(r.status()).toBe(200)
  const body = (await r.json()) as { key: string; size_bytes: number }
  expect(body.key).toMatch(/^backups\/.+\.sql\.gz$/)
  expect(body.size_bytes).toBeGreaterThan(500) // gzipped pg_dump is always non-trivial
})
