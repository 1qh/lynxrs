import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('websocket receives FileCreated broadcast after upload', async ({ browser }) => {
  // Sign up FROM the browser context so the session cookie ends up in the
  // browser's cookie jar — then a same-origin WebSocket upgrade carries it.
  const ctx = await browser.newContext()
  const page = await ctx.newPage()
  const email = `ws-${Date.now()}@example.com`
  const password = 'hunter2hunter2'
  // Use page.request — its cookies live in the BrowserContext, so a
  // subsequent same-origin WebSocket carries them.
  const signup = await page.request.post(`${BACKEND}/api/auth/signup`, {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(signup.ok()).toBe(true)
  const csrf = ((await signup.json()) as { csrf_token?: string }).csrf_token ?? ''
  expect(csrf).toBeTruthy()
  // Park on the frontend dev server so the page origin matches site=localhost
  // (cookies default sameSite=Lax → only sent on same-site navigations).
  // Just need the page to be live; we'll use page.evaluate for fetch+ws.
  await page.goto('http://localhost:3000/main.web.bundle')

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

  // Wait for WS to connect (we'll see ping on connect) then upload from the
  // same browser context so the cookie jar carries the session.
  await page.waitForTimeout(800)
  const up = await page.request.post(`${BACKEND}/api/files/json`, {
    data: {
      filename: 'ws-note.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hello ws').toString('base64'),
    },
    headers: { 'content-type': 'application/json', 'x-csrf-token': csrf },
  })
  expect(up.status()).toBe(201)

  const msgs = await received
  await ctx.close()

  expect(msgs.length).toBeGreaterThanOrEqual(2)
  const parsed = msgs.map((m) => JSON.parse(m))
  expect(parsed.some((m: { kind?: string }) => m.kind === 'ping')).toBe(true)
  expect(parsed.some((m: { kind?: string }) => m.kind === 'file_created')).toBe(true)
})
