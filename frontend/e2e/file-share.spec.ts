import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('file share: create → public download → revoke → 400', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `share-e2e-${Date.now()}@example.com`

  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  const up = await api.post('/api/files/json', {
    data: {
      filename: 'share.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hello share').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  expect(up.status()).toBe(201)
  const file = (await up.json()) as { id: string }

  const shareRes = await api.post(`/api/files/${file.id}/shares`, {
    data: { ttl_hours: 1 },
    headers: { 'content-type': 'application/json' },
  })
  expect(shareRes.status()).toBe(201)
  const share = (await shareRes.json()) as { id: string; url: string }
  const token = share.url.split('/').pop()!

  const anon = await pwRequest.newContext({ baseURL: BACKEND })
  const pub = await anon.get(`/api/shares/${token}`)
  expect(pub.status()).toBe(200)
  expect(await pub.text()).toBe('hello share')

  const rev = await api.delete(`/api/files/shares/${share.id}`)
  expect(rev.status()).toBe(204)

  const post = await anon.get(`/api/shares/${token}`)
  expect(post.status()).toBe(400)
})
