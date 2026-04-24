import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'
const MAILPIT = 'http://localhost:8125'

test('forgot → email → reset → login', async () => {
  const api = await newApi()
  const email = `reset-${Date.now()}@example.com`
  const password = 'hunter2hunter2'

  // signup
  const signup = await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(signup.ok()).toBe(true)

  // forgot password
  const forgot = await api.post('/api/auth/password/forgot', {
    data: { email },
    headers: { 'content-type': 'application/json' },
  })
  expect(forgot.status()).toBe(202)

  // fetch email from mailpit, extract token
  // Mailpit may take a moment; poll briefly.
  let token: string | null = null
  for (let i = 0; i < 30 && !token; i++) {
    const list = await fetch(`${MAILPIT}/api/v1/messages`)
    const data = (await list.json()) as {
      messages: { ID: string; To: { Address: string }[] }[]
    }
    const msg = data.messages.find((m) => m.To.some((t) => t.Address === email))
    if (msg) {
      const full = (await (await fetch(`${MAILPIT}/api/v1/message/${msg.ID}`)).json()) as { Text?: string }
      const match = (full.Text ?? '').match(/token=([A-Za-z0-9_-]+)/)
      if (match?.[1]) token = match[1]
    }
    if (!token) await new Promise((r) => setTimeout(r, 250))
  }
  expect(token).toBeTruthy()

  // reset with new password
  const newPw = 'newstrongpassword'
  const reset = await api.post('/api/auth/password/reset', {
    data: { token, new_password: newPw },
    headers: { 'content-type': 'application/json' },
  })
  expect(reset.status()).toBe(204)

  // fresh login with new password
  const freshCtx = await newApi()
  const login = await freshCtx.post('/api/auth/login', {
    data: { email, password: newPw },
    headers: { 'content-type': 'application/json' },
  })
  expect(login.ok()).toBe(true)

  // old password should fail
  const freshCtx2 = await newApi()
  const badLogin = await freshCtx2.post('/api/auth/login', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(badLogin.status()).toBe(401)
})
