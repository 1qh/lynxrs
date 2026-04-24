import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import http from 'node:http'

const BACKEND = 'http://localhost:8088'

test('OAuth flow with mock provider creates a new account and issues session', async () => {
  // This test expects the backend to be started with OAUTH_* env vars pointing
  // at the mock server launched below. The default backend (without those env
  // vars) returns 400 "OAuth not configured" — which we assert as the status
  // endpoint behavior, so we only run the full flow when configured.
  const api = await newApi()
  const statusRes = await api.get('/api/auth/oauth/status')
  expect(statusRes.status()).toBe(200)
  const status = (await statusRes.json()) as { google: boolean }

  if (!status.google) {
    // Default unconfigured case: /start returns 400.
    const start = await api.get('/api/auth/oauth/google/start', {
      maxRedirects: 0,
    })
    expect(start.status()).toBe(400)
    return
  }

  // Configured case (run separately with env). Flow:
  const mockEmail = `oauth-${Date.now()}@t.local`
  const mock = http.createServer((req, res) => {
    if (req.url?.startsWith('/token')) {
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ access_token: 'mock-tok', token_type: 'Bearer' }))
    } else if (req.url?.startsWith('/userinfo')) {
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end(JSON.stringify({ email: mockEmail }))
    } else {
      res.writeHead(404).end()
    }
  })
  await new Promise<void>((r) => mock.listen(9893, r))

  try {
    const start = await api.get('/api/auth/oauth/google/start', { maxRedirects: 0 })
    expect([302, 307, 308]).toContain(start.status())
    const cookie = (start.headers()['set-cookie'] ?? '').split(';')[0]
    const location = start.headers()['location']!
    const state = new URL(location).searchParams.get('state')!

    const cb = await api.get(
      `/api/auth/oauth/google/callback?code=fake&state=${state}`,
      { maxRedirects: 0, headers: { cookie } },
    )
    expect([302, 307, 308]).toContain(cb.status())

    // Session cookie should now be set — call /me
    const me = await api.get('/api/auth/me')
    expect(me.status()).toBe(200)
    const body = (await me.json()) as { email: string }
    expect(body.email).toBe(mockEmail)
  } finally {
    mock.close()
  }
})
