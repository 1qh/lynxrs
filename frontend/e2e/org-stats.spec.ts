import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('org stats reports members/files/bytes', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `ost-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const slug = `os-${Date.now().toString(36)}`
  const { id: org_id } = (await (await api.post('/api/orgs', {
    data: { name: 'Stats Org', slug },
    headers: { 'content-type': 'application/json' },
  })).json()) as { id: string }

  const payload = Buffer.from('hello org stats').toString('base64')
  await api.post('/api/files/json', {
    data: { filename: 'o.txt', content_type: 'text/plain', data_base64: payload, org_id },
    headers: { 'content-type': 'application/json' },
  })

  const stats = await api.get(`/api/orgs/${org_id}/stats`)
  expect(stats.status()).toBe(200)
  const s = (await stats.json()) as { members: number; files: number; total_bytes: number }
  expect(s.members).toBe(1)
  expect(s.files).toBe(1)
  expect(s.total_bytes).toBe('hello org stats'.length)
})
