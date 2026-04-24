import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('file versioning: snapshot on replace, list, restore', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `ver-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: {
      filename: 'v.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('first').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  // Replace content → snapshot current as v1
  const v2 = await api.post(`/api/files/${id}/versions`, {
    data: {
      filename: 'v.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('second').toString('base64'),
    },
    headers: { 'content-type': 'application/json' },
  })
  expect(v2.status()).toBe(201)

  // Download current → "second"
  expect(await (await api.get(`/api/files/${id}`)).text()).toBe('second')

  // List versions: [v1=first]
  const list = (await (await api.get(`/api/files/${id}/versions`)).json()) as { version_no: number }[]
  expect(list).toHaveLength(1)
  expect(list[0]!.version_no).toBe(1)

  // Restore v1 (which also snapshots current as v2)
  const restored = await api.post(`/api/files/${id}/versions/1/restore`)
  expect(restored.status()).toBe(200)
  expect(await (await api.get(`/api/files/${id}`)).text()).toBe('first')

  const list2 = (await (await api.get(`/api/files/${id}/versions`)).json()) as { version_no: number }[]
  expect(list2.map((v) => v.version_no).sort()).toEqual([1, 2])
})
