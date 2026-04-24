import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import http from 'node:http'

const BACKEND = 'http://localhost:8088'

test('webhook deliveries are logged per attempt', async () => {
  const server = http.createServer((_req, res) => {
    res.writeHead(204).end()
  })
  await new Promise<void>((r) => server.listen(9892, r))

  try {
    const api = await newApi()
    const email = `whd-${Date.now()}@t.local`
    await api.post('/api/auth/signup', {
      data: { email, password: 'hunter2hunter2' },
      headers: { 'content-type': 'application/json' },
    })
    const whRes = await api.post('/api/webhooks', {
      data: { url: 'http://localhost:9892/hook' },
      headers: { 'content-type': 'application/json' },
    })
    const { webhook } = (await whRes.json()) as { webhook: { id: string } }

    await api.post('/api/files/json', {
      data: { filename: 'x.txt', content_type: 'text/plain', data_base64: 'aGk=' },
      headers: { 'content-type': 'application/json' },
    })

    // Wait for delivery log row.
    await expect.poll(async () => {
      const r = await api.get(`/api/webhooks/${webhook.id}/deliveries`)
      const rows = (await r.json()) as { status: number | null; event_kind: string }[]
      return rows.length
    }, { timeout: 5000 }).toBeGreaterThan(0)

    const rows = (await (await api.get(`/api/webhooks/${webhook.id}/deliveries`)).json()) as {
      status: number | null
      attempt: number
      event_kind: string
    }[]
    expect(rows[0]!.status).toBe(204)
    expect(rows[0]!.attempt).toBe(1)
    expect(rows[0]!.event_kind).toBe('file_created')
  } finally {
    server.close()
  }
})
