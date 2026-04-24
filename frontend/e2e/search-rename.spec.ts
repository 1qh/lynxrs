import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

async function signup(api: Awaited<ReturnType<typeof pwRequest.newContext>>) {
  await api.post('/api/auth/signup', {
    data: { email: `sr-${Date.now()}-${Math.random()}@t.local`, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
}

async function upload(api: Awaited<ReturnType<typeof pwRequest.newContext>>, filename: string) {
  const res = await api.post('/api/files/json', {
    data: { filename, content_type: 'text/plain', data_base64: 'aGk=' },
    headers: { 'content-type': 'application/json' },
  })
  return (await res.json()) as { id: string; filename: string }
}

test('file search filters by filename (case-insensitive)', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  await signup(api)
  await upload(api, 'alpha.txt')
  await upload(api, 'beta.txt')
  await upload(api, 'alphabet.txt')

  const res = await api.get('/api/files?q=ALPHA')
  expect(res.status()).toBe(200)
  const body = (await res.json()) as { items: { filename: string }[] }
  const names = body.items.map((f) => f.filename).sort()
  expect(names).toEqual(['alpha.txt', 'alphabet.txt'])
})

test('file rename updates filename', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  await signup(api)
  const f = await upload(api, 'orig.txt')

  const res = await api.patch(`/api/files/${f.id}`, {
    data: { filename: 'renamed.txt' },
    headers: { 'content-type': 'application/json' },
  })
  expect(res.status()).toBe(200)
  const body = (await res.json()) as { filename: string }
  expect(body.filename).toBe('renamed.txt')
})
