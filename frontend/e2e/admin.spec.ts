import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('admin endpoints are role-gated', async () => {
  const api = await newApi()
  const email = `admin-e2e-${Date.now()}@example.com`
  const password = 'hunter2hunter2'

  const signup = await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(signup.ok()).toBe(true)

  // Fresh signup has role=user; admin/stats must 401.
  const unauth = await api.get('/api/admin/stats')
  expect(unauth.status()).toBe(401)

  const unauthUsers = await api.get('/api/admin/users')
  expect(unauthUsers.status()).toBe(401)
})
