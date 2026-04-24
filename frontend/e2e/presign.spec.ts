import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('GET /files/{id}/presign returns a working S3 URL', async () => {
  const api = await newApi()
  const email = `pre-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: {
      filename: 'p.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('presigned!').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  const res = await api.get(`/api/files/${id}/presign`)
  expect(res.status()).toBe(200)
  const { url, expires_in_seconds } = (await res.json()) as { url: string; expires_in_seconds: number }
  expect(url).toContain('X-Amz-Signature')
  expect(expires_in_seconds).toBeGreaterThan(0)

  // Anonymous GET with the presigned URL must work.
  const anon = await newApi()
  const fetched = await anon.get(url)
  expect(fetched.status()).toBe(200)
  expect(await fetched.text()).toBe('presigned!')
})
