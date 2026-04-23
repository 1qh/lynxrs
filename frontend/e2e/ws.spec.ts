import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('websocket receives FileCreated broadcast after upload', async ({ browser }) => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `ws-${Date.now()}@example.com`
  const password = 'hunter2hunter2'

  const signup = await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(signup.ok()).toBe(true)

  const state = await api.storageState()
  const sessionCookie = state.cookies.find((c) => c.name === 'simu_session')
  expect(sessionCookie).toBeTruthy()

  // Give the browser page the session cookie so WS upgrade carries it.
  const ctx = await browser.newContext()
  await ctx.addCookies([
    {
      name: 'simu_session',
      value: sessionCookie!.value,
      domain: 'localhost',
      path: '/',
      httpOnly: true,
      secure: false,
      sameSite: 'Lax',
    },
  ])
  const page = await ctx.newPage()
  // Navigate once so WebSocket opens from a http://localhost origin.
  await page.goto(`${BACKEND}/health`)

  const wsUrl = 'ws://localhost:8088/events/ws'
  const received = page.evaluate((wsUrl: string) => {
    return new Promise<string[]>((resolve) => {
      const msgs: string[] = []
      const ws = new WebSocket(wsUrl)
      ws.onmessage = (e) => {
        msgs.push(String(e.data))
      }
      ws.onerror = () => resolve(msgs)
      ws.onclose = () => resolve(msgs)
      setTimeout(() => {
        try { ws.close() } catch {}
        resolve(msgs)
      }, 6000)
    })
  }, wsUrl)

  // Wait for WS to connect (we'll see ping on connect) then upload.
  await page.waitForTimeout(800)
  const up = await api.post('/api/files/json', {
    data: {
      filename: 'ws-note.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hello ws').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  expect(up.ok()).toBe(true)

  const msgs = await received
  await ctx.close()

  expect(msgs.length).toBeGreaterThanOrEqual(2)
  const parsed = msgs.map((m) => JSON.parse(m))
  expect(parsed.some((m: { kind?: string }) => m.kind === 'ping')).toBe(true)
  expect(parsed.some((m: { kind?: string }) => m.kind === 'file_created')).toBe(true)
})
