import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import http from 'node:http'
import crypto from 'node:crypto'

const BACKEND = 'http://localhost:8088'

test('webhook fires on file_created with hmac-sha256 signature', async () => {
  // Local receiver.
  const received: { body: string; sig: string }[] = []
  const server = http.createServer((req, res) => {
    let buf = ''
    req.on('data', (c) => (buf += c))
    req.on('end', () => {
      received.push({ body: buf, sig: String(req.headers['x-simu-signature'] ?? '') })
      res.writeHead(204).end()
    })
  })
  await new Promise<void>((r) => server.listen(9891, r))

  try {
    const api = await newApi()
    const email = `wh-${Date.now()}@t.local`
    await api.post('/api/auth/signup', {
      data: { email, password: 'hunter2hunter2' },
      headers: { 'content-type': 'application/json' },
    })
    const whRes = await api.post('/api/webhooks', {
      data: { url: 'http://host.docker.internal:9891/hook'.replace('host.docker.internal', 'localhost') },
      headers: { 'content-type': 'application/json' },
    })
    expect(whRes.status()).toBe(201)
    const { secret } = (await whRes.json()) as { secret: string }

    const up = await api.post('/api/files/json', {
      data: { filename: 'hook.txt', content_type: 'text/plain', data_base64: 'aGk=' },
      headers: { 'content-type': 'application/json' },
    })
    expect(up.status()).toBe(201)

    // Wait for dispatch.
    await expect.poll(() => received.length, { timeout: 5000 }).toBeGreaterThan(0)

    const hit = received[0]!
    const expected = 'sha256=' + crypto.createHmac('sha256', secret).update(hit.body).digest('hex')
    expect(hit.sig).toBe(expected)
    const payload = JSON.parse(hit.body) as { kind: string }
    expect(payload.kind).toBe('file_created')
  } finally {
    server.close()
  }
})
