// Minimal k6 load test for simu backend.
// Run: k6 run ops/load.k6.js
// Ramps a cohort of users through signup → me → list. Safe to run against dev.

import http from 'k6/http'
import encoding from 'k6/encoding'
import { check, sleep } from 'k6'

export const options = {
  stages: [
    { duration: '10s', target: 20 },
    { duration: '30s', target: 50 },
    { duration: '10s', target: 0 },
  ],
  thresholds: {
    http_req_failed: ['rate<0.02'],
    http_req_duration: ['p(95)<300', 'p(99)<800'],
  },
}

const BASE = __ENV.SIMU_BASE || 'http://127.0.0.1:8088'

export default function () {
  const email = `k6-${__VU}-${__ITER}-${Date.now()}@t.com`
  const password = 'hunter2hunter2'
  const jar = http.cookieJar()

  const signup = http.post(
    `${BASE}/api/auth/signup`,
    JSON.stringify({ email, password }),
    { headers: { 'Content-Type': 'application/json' }, jar },
  )
  check(signup, { 'signup 201': (r) => r.status === 201 })

  const me = http.get(`${BASE}/api/auth/me`, { jar })
  check(me, { 'me 200': (r) => r.status === 200 })

  const list = http.get(`${BASE}/api/files`, { jar })
  check(list, { 'list 200': (r) => r.status === 200 })

  // Upload
  const data = encoding.b64encode('hello k6 load'.repeat(32))
  const upload = http.post(
    `${BASE}/api/files/json`,
    JSON.stringify({ filename: `k6-${__VU}-${__ITER}.txt`, content_type: 'text/plain', data_base64: data }),
    { headers: { 'Content-Type': 'application/json' }, jar },
  )
  check(upload, { 'upload 201': (r) => r.status === 201 })
  const fid = upload.json('id')

  const head = http.request('HEAD', `${BASE}/api/files/${fid}`, null, { jar })
  check(head, { 'head 200': (r) => r.status === 200 })

  const presign = http.get(`${BASE}/api/files/${fid}/presign`, { jar })
  check(presign, { 'presign 200': (r) => r.status === 200 })

  http.post(`${BASE}/api/files/${fid}/tags`, JSON.stringify({ tag: 'load' }),
    { headers: { 'Content-Type': 'application/json' }, jar })
  const stats = http.get(`${BASE}/api/me/stats`, { jar })
  check(stats, { 'stats 200': (r) => r.status === 200 })

  const share = http.post(
    `${BASE}/api/files/${fid}/shares`,
    JSON.stringify({}),
    { headers: { 'Content-Type': 'application/json' }, jar },
  )
  check(share, { 'share 201': (r) => r.status === 201 })

  const logout = http.post(`${BASE}/api/auth/logout`, null, { jar })
  check(logout, { 'logout 204': (r) => r.status === 204 })

  sleep(1)
}
