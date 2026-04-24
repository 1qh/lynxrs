import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('presigned PUT upload — direct-to-S3, then confirm', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `pu-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  const body = 'hello direct upload!'
  const presign = await api.post('/api/files/presign-upload', {
    data: {
      filename: 'direct.txt',
      content_type: 'text/plain',
      size_bytes: body.length,
    },
    headers: { 'content-type': 'application/json' },
  })
  expect(presign.status()).toBe(200)
  const { file_id, put_url } = (await presign.json()) as { file_id: string; put_url: string }

  // Anonymous PUT direct to S3.
  const anon = await pwRequest.newContext()
  const put = await anon.fetch(put_url, {
    method: 'PUT',
    data: body,
    headers: { 'content-type': 'text/plain' },
  })
  expect(put.status()).toBe(200)

  const confirm = await api.post(`/api/files/${file_id}/confirm-upload`)
  expect(confirm.status()).toBe(200)

  // Download through backend confirms the object is real.
  const down = await api.get(`/api/files/${file_id}`)
  expect(await down.text()).toBe(body)
})
