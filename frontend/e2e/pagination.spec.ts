import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('/api/files cursor pagination walks backwards through history', async () => {
  const api = await pwRequest.newContext({ baseURL: BACKEND })
  const email = `paginate-${Date.now()}@example.com`
  await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  // Create 5 files in order p1..p5 (oldest..newest).
  const names: string[] = []
  for (let i = 1; i <= 5; i++) {
    const fn = `page-${Date.now()}-${i}.txt`
    names.push(fn)
    await api.post('/api/files/json', {
      data: {
        filename: fn,
        content_type: 'text/plain',
        data_base64: Buffer.from(`p${i}`).toString('base64'),
      },
      headers: { 'content-type': 'application/json' },
    })
    await new Promise((r) => setTimeout(r, 20)) // ensure distinct created_at
  }

  type Page = { items: { filename: string }[]; next_cursor: string | null }

  // Page 1 (limit=2): newest two (p5, p4)
  const p1 = (await (await api.get('/api/files?limit=2')).json()) as Page
  expect(p1.items).toHaveLength(2)
  expect(p1.items[0]!.filename).toBe(names[4]) // p5
  expect(p1.items[1]!.filename).toBe(names[3]) // p4
  expect(p1.next_cursor).toBeTruthy()

  const p2 = (await (
    await api.get(`/api/files?limit=2&cursor=${encodeURIComponent(p1.next_cursor!)}`)
  ).json()) as Page
  expect(p2.items).toHaveLength(2)
  expect(p2.items[0]!.filename).toBe(names[2]) // p3
  expect(p2.items[1]!.filename).toBe(names[1]) // p2
  expect(p2.next_cursor).toBeTruthy()

  const p3 = (await (
    await api.get(`/api/files?limit=2&cursor=${encodeURIComponent(p2.next_cursor!)}`)
  ).json()) as Page
  expect(p3.items).toHaveLength(1)
  expect(p3.items[0]!.filename).toBe(names[0]) // p1
  expect(p3.next_cursor).toBeNull()
})
