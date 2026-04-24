import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('audit log records signup + login', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `audit-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  await api.post('/api/auth/login', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  const res = await api.get('/api/me/audit')
  expect(res.status()).toBe(200)
  const rows = (await res.json()) as { action: string }[]
  const actions = rows.map((r) => r.action)
  expect(actions).toContain('login')
  expect(actions).toContain('signup')
})

test('audit log records failed login (by email)', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `audit-fail-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const bad = await pwRequest.newContext({ baseURL: BACKEND })
  const r = await bad.post('/api/auth/login', {
    data: { email, password: 'wrong-wrong-wrong' },
    headers: { 'content-type': 'application/json' },
  })
  expect(r.status()).toBe(401)

  const res = await api.get('/api/me/audit')
  const rows = (await res.json()) as { action: string }[]
  expect(rows.map((r) => r.action)).toContain('login_failed')
})
