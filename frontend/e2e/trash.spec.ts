import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const BACKEND = 'http://localhost:8088'

test('soft-delete → list-trash → restore → delete → purge', async () => {
  const api = await newApi()
  const email = `trash-${Date.now()}@t.local`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  const up = await api.post('/api/files/json', {
    data: { filename: 't.txt', content_type: 'text/plain', data_base64: 'aGk=' },
    headers: { 'content-type': 'application/json' },
  })
  const { id } = (await up.json()) as { id: string }

  // Soft delete
  expect((await api.delete(`/api/files/${id}`)).status()).toBe(204)

  // No longer in active list
  const list1 = (await (await api.get('/api/files')).json()) as { items: { id: string }[] }
  expect(list1.items.find((f) => f.id === id)).toBeUndefined()

  // In trash
  const trash = (await (await api.get('/api/trash')).json()) as { items: { id: string }[] }
  expect(trash.items.find((f) => f.id === id)).toBeTruthy()

  // Restore
  expect((await api.post(`/api/trash/${id}/restore`)).status()).toBe(204)
  const list2 = (await (await api.get('/api/files')).json()) as { items: { id: string }[] }
  expect(list2.items.find((f) => f.id === id)).toBeTruthy()

  // Delete again + purge
  expect((await api.delete(`/api/files/${id}`)).status()).toBe(204)
  expect((await api.delete(`/api/trash/${id}`)).status()).toBe(204)
  const trash2 = (await (await api.get('/api/trash')).json()) as { items: { id: string }[] }
  expect(trash2.items.find((f) => f.id === id)).toBeUndefined()
})
