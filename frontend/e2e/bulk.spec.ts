import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('bulk tag → untag → delete', async () => {
  const api = await newApi()
  const email = `bulk-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const ids: string[] = []
  for (const name of ['b1.txt', 'b2.txt', 'b3.txt']) {
    const up = await api.post('/api/files/json', {
      data: { filename: name, content_type: 'text/plain', data_base64: 'aGk=' },
      headers: { 'content-type': 'application/json' },
    })
    const { id } = (await up.json()) as { id: string }
    ids.push(id)
  }

  const tag = await api.post('/api/files/bulk', {
    data: { action: 'tag', ids, tag: 'archive' },
    headers: { 'content-type': 'application/json' },
  })
  expect((await tag.json()).affected).toBe(3)

  const list = (await (await api.get('/api/files?tag=archive')).json()) as { items: unknown[] }
  expect(list.items).toHaveLength(3)

  const untag = await api.post('/api/files/bulk', {
    data: { action: 'untag', ids, tag: 'archive' },
    headers: { 'content-type': 'application/json' },
  })
  expect((await untag.json()).affected).toBe(3)

  const del = await api.post('/api/files/bulk', {
    data: { action: 'delete', ids },
    headers: { 'content-type': 'application/json' },
  })
  expect((await del.json()).affected).toBe(3)
})
