import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('account locks after 5 failed logins', async () => {
  const anon = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `lock-${Date.now()}@t.local`
  const pw = 'hunter2hunter2'

  await anon.post('/api/auth/signup', {
    data: { email, password: pw },
    headers: { 'content-type': 'application/json' },
  })

  for (let i = 0; i < 5; i++) {
    const r = await anon.post('/api/auth/login', {
      data: { email, password: 'wrong-wrong-wrong' },
      headers: { 'content-type': 'application/json' },
    })
    expect(r.status()).toBe(401)
  }

  // Now correct password should still be rejected (locked).
  const locked = await anon.post('/api/auth/login', {
    data: { email, password: pw },
    headers: { 'content-type': 'application/json' },
  })
  expect(locked.status()).toBe(401)
})
