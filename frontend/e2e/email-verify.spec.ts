import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'
const MAILPIT = 'http://localhost:8125'

test('signup auto-sends verification email; verify endpoint marks user verified', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `verify-${Date.now()}@example.com`

  const signup = await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  expect(signup.ok()).toBe(true)

  // Poll Mailpit for the verification email.
  let token: string | null = null
  for (let i = 0; i < 30 && !token; i++) {
    const list = (await (await fetch(`${MAILPIT}/api/v1/messages`)).json()) as {
      messages: { ID: string; Subject: string; To: { Address: string }[] }[]
    }
    const msg = list.messages.find(
      (m) => m.Subject.includes('Confirm') && m.To.some((t) => t.Address === email),
    )
    if (msg) {
      const full = (await (
        await fetch(`${MAILPIT}/api/v1/message/${msg.ID}`)
      ).json()) as { Text?: string }
      const match = (full.Text ?? '').match(/token=([A-Za-z0-9_-]+)/)
      if (match?.[1]) token = match[1]
    }
    if (!token) await new Promise((r) => setTimeout(r, 250))
  }
  expect(token).toBeTruthy()

  const verify = await api.post('/api/auth/email/verify', {
    data: { token },
    headers: { 'content-type': 'application/json' },
  })
  expect(verify.status()).toBe(204)

  // Using the same token again → 400.
  const again = await api.post('/api/auth/email/verify', {
    data: { token },
    headers: { 'content-type': 'application/json' },
  })
  expect(again.status()).toBe(400)
})
