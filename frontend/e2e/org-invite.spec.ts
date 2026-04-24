import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('org invite flow: create invite → preview → accept → membership visible', async () => {
  const owner = await pwRequest.newContext({ baseURL: BACKEND })
  const invitee = await pwRequest.newContext({ baseURL: BACKEND })
  const ownerEmail = `inv-o-${Date.now()}@t.local`
  const inviteeEmail = `inv-i-${Date.now()}@t.local`
  await owner.post('/api/auth/signup', { data: { email: ownerEmail, password: 'hunter2hunter2' }, headers: { 'content-type': 'application/json' } })
  await invitee.post('/api/auth/signup', { data: { email: inviteeEmail, password: 'hunter2hunter2' }, headers: { 'content-type': 'application/json' } })

  const slug = `inv-${Date.now().toString(36)}`
  const { id: org_id } = (await (await owner.post('/api/orgs', {
    data: { name: 'Invite Org', slug },
    headers: { 'content-type': 'application/json' },
  })).json()) as { id: string }

  const inviteRes = await owner.post(`/api/orgs/${org_id}/invites`, {
    data: { email: inviteeEmail },
    headers: { 'content-type': 'application/json' },
  })
  expect(inviteRes.status()).toBe(201)
  const { url } = (await inviteRes.json()) as { url: string }
  const token = new URL(url).searchParams.get('token')!

  // Unauthenticated preview works
  const anon = await pwRequest.newContext({ baseURL: BACKEND })
  const pv = await anon.get(`/api/invites/${token}`)
  expect(pv.status()).toBe(200)
  const preview = (await pv.json()) as { email: string }
  expect(preview.email).toBe(inviteeEmail)

  // Accept as invitee
  const accept = await invitee.post(`/api/invites/${token}/accept`)
  expect(accept.status()).toBe(200)

  // Invitee sees the org
  const invitedOrgs = (await (await invitee.get('/api/orgs')).json()) as { id: string }[]
  expect(invitedOrgs.some((o) => o.id === org_id)).toBe(true)

  // Same token can't be accepted again
  const again = await invitee.post(`/api/invites/${token}/accept`)
  expect(again.status()).toBe(404)
})
