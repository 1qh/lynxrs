import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('DELETE /api/auth/me permanently removes the user', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `delete-${Date.now()}@example.com`
  const password = 'hunter2hunter2'

  await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })

  const del = await api.delete('/api/auth/me')
  expect(del.status()).toBe(204)

  // me → 401 (cookie cleared)
  const me = await api.get('/api/auth/me')
  expect(me.status()).toBe(401)

  // login → 401 (user gone)
  const fresh = await pwRequest.newContext({ baseURL: BACKEND })
  const login = await fresh.post('/api/auth/login', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(login.status()).toBe(401)
})
