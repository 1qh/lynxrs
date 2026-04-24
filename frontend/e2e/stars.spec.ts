import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('star + list-starred + unstar', async () => {
  const api = await newApi()
  const email = `star-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: { filename: 's.txt', content_type: 'text/plain', data_base64: 'aGk=' },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  expect((await api.post(`/api/files/${id}/star`)).status()).toBe(204)
  const list = (await (await api.get('/api/files/starred')).json()) as { id: string }[]
  expect(list.map((f) => f.id)).toContain(id)

  expect((await api.delete(`/api/files/${id}/star`)).status()).toBe(204)
  const list2 = (await (await api.get('/api/files/starred')).json()) as { id: string }[]
  expect(list2.map((f) => f.id)).not.toContain(id)
})
