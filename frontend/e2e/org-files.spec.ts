import { test, expect, request as pwRequest } from '@playwright/test'

const BACKEND = 'http://localhost:8088'

test('org-scoped upload is visible to other org members; non-members see 404', async () => {
  const owner = await pwRequest.newContext({ baseURL: BACKEND })
  const member = await pwRequest.newContext({ baseURL: BACKEND })
  const stranger = await pwRequest.newContext({ baseURL: BACKEND })
  const ownerEmail = `org-own-${Date.now()}@t.local`
  const memberEmail = `org-mbr-${Date.now()}@t.local`
  await owner.post('/api/auth/signup', { data: { email: ownerEmail, password: 'hunter2hunter2' }, headers: { 'content-type': 'application/json' } })
  await member.post('/api/auth/signup', { data: { email: memberEmail, password: 'hunter2hunter2' }, headers: { 'content-type': 'application/json' } })
  await stranger.post('/api/auth/signup', { data: { email: `str-${Date.now()}@t.local`, password: 'hunter2hunter2' }, headers: { 'content-type': 'application/json' } })

  const slug = `o-${Date.now().toString(36)}`
  const { id: org_id } = (await (await owner.post('/api/orgs', {
    data: { name: 'Files Org', slug },
    headers: { 'content-type': 'application/json' },
  })).json()) as { id: string }

  await owner.post(`/api/orgs/${org_id}/members`, {
    data: { email: memberEmail },
    headers: { 'content-type': 'application/json' },
  })

  const up = await owner.post('/api/files/json', {
    data: {
      filename: 'org.txt',
      content_type: 'text/plain',
      data_base64: Buffer.from('hello org').toString('base64'),
      org_id,
    },
    headers: { 'content-type': 'application/json' },
  })
  expect(up.status()).toBe(201)
  const { id: fid } = (await up.json()) as { id: string }

  // Member can download
  const memberDl = await member.get(`/api/files/${fid}`)
  expect(memberDl.status()).toBe(200)
  expect(await memberDl.text()).toBe('hello org')

  // Member sees it in listing
  const list = (await (await member.get('/api/files')).json()) as { items: { id: string }[] }
  expect(list.items.some((f) => f.id === fid)).toBe(true)

  // Stranger gets 404
  const nope = await stranger.get(`/api/files/${fid}`)
  expect(nope.status()).toBe(404)
})
