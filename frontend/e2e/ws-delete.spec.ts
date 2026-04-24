import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import WebSocket from 'ws'

const BACKEND = 'http://localhost:8088'

test('WebSocket receives file_deleted event', async () => {
  const api = await newApi()
  const email = `wsdel-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const state = await api.storageState()
  const session = state.cookies.find((c) => c.name === 'simu_session')
  expect(session).toBeTruthy()

  const up = await api.post('/api/files/json', {
    data: { filename: 'wd.txt', content_type: 'text/plain', data_base64: 'aGk=' },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  const messages: string[] = []
  const ws = new WebSocket('ws://localhost:8088/events/ws', {
    headers: { cookie: `simu_session=${session!.value}` },
  })
  await new Promise<void>((r) => ws.on('open', () => r()))
  ws.on('message', (m) => messages.push(m.toString()))

  await api.delete(`/api/files/${id}`)
  await expect.poll(() => messages.some((m) => m.includes('file_deleted')), { timeout: 3000 }).toBe(true)
  ws.close()
})
