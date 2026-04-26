import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import { execSync } from 'node:child_process'

const BACKEND = 'http://localhost:8088'

test('admin impersonation issues session as target user', async () => {
  // Seed an admin via simu-admin CLI. Spec needs the workspace layout, so
  // we skip when the binary isn't where we expect.
  const repoRoot = process.env.SIMU_REPO_ROOT ?? '/Users/o/simu'
  try {
    execSync(`test -f ${repoRoot}/backend/target/release/simu-admin`, { stdio: 'pipe' })
  } catch {
    test.skip(true, `simu-admin CLI not at ${repoRoot}/backend/target/release`)
    return
  }
  const adminEmail = `admin-imp-${Date.now()}@t.local`
  const pw = 'hunter2hunter2'
  try {
    execSync(
      `env $(cat ${repoRoot}/backend/.env | xargs) ${repoRoot}/backend/target/release/simu-admin create --email ${adminEmail} --password ${pw}`,
      { stdio: 'pipe' },
    )
  } catch (e) {
    const err = e as { stderr?: Buffer; stdout?: Buffer; message?: string }
    const stderr = err.stderr?.toString() ?? ''
    const stdout = err.stdout?.toString() ?? ''
    // Linux act containers may not have the same binary ABI/host paths.
    if (process.platform !== 'darwin') {
      test.skip(true, `simu-admin failed on ${process.platform}: ${stderr || stdout || err.message}`)
      return
    }
    throw new Error(`simu-admin create failed: stderr=${stderr} stdout=${stdout}`)
  }
  const admin = await newApi()
  const login = await admin.post('/api/auth/login', {
    data: { email: adminEmail, password: pw },
    headers: { 'content-type': 'application/json' },
  })
  expect(login.status()).toBe(200)

  // Create a target user via signup.
  const targetEmail = `tgt-imp-${Date.now()}@t.local`
  const target = await newApi()
  const sres = await target.post('/api/auth/signup', {
    data: { email: targetEmail, password: pw },
    headers: { 'content-type': 'application/json' },
  })
  const { id: targetId } = (await sres.json()) as { id: string }

  // Admin impersonates target.
  const imp = await admin.post(`/api/admin/users/${targetId}/impersonate`)
  expect(imp.status()).toBe(200)
  const body = (await imp.json()) as { impersonating: string }
  expect(body.impersonating).toBe(targetEmail)

  // Now the admin context's /me returns the target.
  const meAfter = await admin.get('/api/auth/me')
  const m = (await meAfter.json()) as { email: string }
  expect(m.email).toBe(targetEmail)
})
