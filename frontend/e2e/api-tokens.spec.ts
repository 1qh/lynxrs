import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('API token create + use as Bearer + revoke', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `token-e2e-${Date.now()}@example.com`

  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  // Create token
  const createRes = await api.post('/api/tokens', {
    data: { name: 'e2e-script' },
    headers: { 'content-type': 'application/json' },
  })
  expect(createRes.status()).toBe(201)
  const body = (await createRes.json()) as { token: { id: string }; plaintext: string }
  expect(body.plaintext).toMatch(/^simu_/)

  // Use bearer token — no cookies attached
  const bareApi = await pwRequest.newContext({ baseURL: BACKEND })
  const listRes = await bareApi.get('/api/files', {
    headers: { Authorization: `Bearer ${body.plaintext}` },
  })
  expect(listRes.status()).toBe(200)

  // Without auth → 401
  const naked = await pwRequest.newContext({ baseURL: BACKEND })
  const unauth = await naked.get('/api/files')
  expect(unauth.status()).toBe(401)

  // Revoke token
  const revoke = await api.delete(`/api/tokens/${body.token.id}`)
  expect(revoke.status()).toBe(204)

  // Revoked token → 401
  const after = await bareApi.get('/api/files', {
    headers: { Authorization: `Bearer ${body.plaintext}` },
  })
  expect(after.status()).toBe(401)
})

test('bearer token works on upload + download flow (no cookie)', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `token-upload-${Date.now()}@example.com`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const created = (await (
    await api.post('/api/tokens', {
      data: { name: 'upload-bot' },
      headers: { 'content-type': 'application/json' },
    })
  ).json()) as { plaintext: string }
  const token = created.plaintext

  const bare = await pwRequest.newContext({ baseURL: BACKEND })

  const up = await bare.post('/api/files/json', {
    data: {
      filename: 'bearer-upload.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hi from bearer').toString('base64'),
    },
    headers: {
      Authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
  })
  expect(up.status()).toBe(201)
  const fileDto = (await up.json()) as { id: string }

  const list = (await (
    await bare.get('/api/files', {
      headers: { Authorization: `Bearer ${token}` },
    })
  ).json()) as { items: { id: string }[] }
  expect(list.items.some((f) => f.id === fileDto.id)).toBe(true)

  const del = await bare.delete(`/api/files/${fileDto.id}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(del.status()).toBe(204)
})
