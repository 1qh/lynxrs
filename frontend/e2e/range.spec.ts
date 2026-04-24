import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('download supports HTTP Range → 206 Partial Content', async () => {
  const api = await newApi()
  const email = `range-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: {
      filename: 'r.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hello world').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  const res = await api.get(`/api/files/${id}`, {
    headers: { Range: 'bytes=6-10' },
  })
  expect(res.status()).toBe(206)
  expect(res.headers()['content-range']).toBe('bytes 6-10/11')
  expect(await res.text()).toBe('world')
})
