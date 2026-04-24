import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('ZIP download bundles multiple files', async () => {
  const api = await newApi()
  const email = `zip-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const ids: string[] = []
  for (const [name, body] of [['a.txt', 'hello'], ['b.txt', 'world']] as const) {
    const up = await api.post('/api/files/json', {
      data: { filename: name, content_type: 'text/plain', data_base64: Buffer.from(body).toString('base64') },
      headers: { 'content-type': 'application/json' },
    })
    ids.push(((await up.json()) as { id: string }).id)
  }
  const zip = await api.post('/api/files/download-zip', {
    data: { ids },
    headers: { 'content-type': 'application/json' },
  })
  expect(zip.status()).toBe(200)
  expect(zip.headers()['content-type']).toBe('application/zip')
  const bytes = await zip.body()
  // ZIP magic: PK\x03\x04
  expect(bytes[0]).toBe(0x50)
  expect(bytes[1]).toBe(0x4b)
  expect(bytes[2]).toBe(0x03)
  expect(bytes[3]).toBe(0x04)
})
