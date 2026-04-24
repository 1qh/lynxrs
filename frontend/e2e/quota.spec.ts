import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('quota: reports usage, grows on upload', async () => {
  const api = await newApi()
  const email = `quota-e2e-${Date.now()}@example.com`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  const q0 = await api.get('/api/me/quota')
  expect(q0.status()).toBe(200)
  const before = (await q0.json()) as { used_bytes: number; limit_bytes: number }
  expect(before.used_bytes).toBe(0)
  expect(before.limit_bytes).toBeGreaterThan(0)

  const payload = Buffer.from('x'.repeat(1234)).toString('base64')
  const up = await api.post('/api/files/json', {
    data: { filename: 'q.txt', content_type: 'text/plain', data_base64: payload },
    headers: { 'content-type': 'application/json' },
  })
  expect(up.status()).toBe(201)

  const q1 = await api.get('/api/me/quota')
  const after = (await q1.json()) as { used_bytes: number }
  expect(after.used_bytes).toBe(1234)
})
