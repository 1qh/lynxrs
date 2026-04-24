import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('file describe + move to org + move back to personal', async () => {
  const api = await newApi()
  const email = `dm-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: { filename: 'd.txt', content_type: 'text/plain', data_base64: 'aGk=' },
    headers: { 'content-type': 'application/json' },
  })
  const { id: fid } = (await up.json()) as { id: string }

  const desc = await api.patch(`/api/files/${fid}/describe`, {
    data: { description: 'My important doc' },
    headers: { 'content-type': 'application/json' },
  })
  expect(desc.status()).toBe(200)
  expect(((await desc.json()) as { description: string }).description).toBe('My important doc')

  // Create org + move file there
  const slug = `mv-${Date.now().toString(36)}`
  const { id: org_id } = (await (await api.post('/api/orgs', {
    data: { name: 'Move Org', slug },
    headers: { 'content-type': 'application/json' },
  })).json()) as { id: string }

  const mv = await api.patch(`/api/files/${fid}/move`, {
    data: { org_id },
    headers: { 'content-type': 'application/json' },
  })
  expect(mv.status()).toBe(200)
  expect(((await mv.json()) as { org_id: string }).org_id).toBe(org_id)

  const mvBack = await api.patch(`/api/files/${fid}/move`, {
    data: { org_id: null },
    headers: { 'content-type': 'application/json' },
  })
  expect(mvBack.status()).toBe(200)
  expect(((await mvBack.json()) as { org_id: string | null }).org_id).toBeNull()
})
