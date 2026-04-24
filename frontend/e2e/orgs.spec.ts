import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('orgs: create → list → invite member → list members → remove', async () => {
  const owner = await pwRequest.newContext({ baseURL: BACKEND })
  const member = await pwRequest.newContext({ baseURL: BACKEND })
  const ownerEmail = `own-${Date.now()}@t.local`
  const memberEmail = `mbr-${Date.now()}@t.local`
  await owner.post('/api/auth/signup', {
    data: { email: ownerEmail, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  await member.post('/api/auth/signup', {
    data: { email: memberEmail, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })

  const slug = `t-${Date.now().toString(36)}`
  const createRes = await owner.post('/api/orgs', {
    data: { name: 'Test Org', slug },
    headers: { 'content-type': 'application/json' },
  })
  expect(createRes.status()).toBe(201)
  const { id: org_id } = (await createRes.json()) as { id: string }

  const list = (await (await owner.get('/api/orgs')).json()) as { slug: string }[]
  expect(list.some((o) => o.slug === slug)).toBe(true)

  const invite = await owner.post(`/api/orgs/${org_id}/members`, {
    data: { email: memberEmail },
    headers: { 'content-type': 'application/json' },
  })
  expect(invite.status()).toBe(201)

  const members = (await (await owner.get(`/api/orgs/${org_id}/members`)).json()) as { email: string }[]
  expect(members.map((m) => m.email).sort()).toEqual([memberEmail, ownerEmail].sort())

  // Non-member cannot list
  const stranger = await pwRequest.newContext({ baseURL: BACKEND })
  await stranger.post('/api/auth/signup', {
    data: { email: `stranger-${Date.now()}@t.local`, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  expect((await stranger.get(`/api/orgs/${org_id}/members`)).status()).toBe(404)

  // Member can see the org in their list
  const memberList = (await (await member.get('/api/orgs')).json()) as { id: string }[]
  expect(memberList.some((o) => o.id === org_id)).toBe(true)

  // Remove member
  const memberRow = members.find((m) => m.email === memberEmail)!
  const rm = await owner.delete(`/api/orgs/${org_id}/members/${(memberRow as any).user_id}`)
  expect(rm.status()).toBe(204)
})
