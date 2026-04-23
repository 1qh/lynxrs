// Minimal k6 load test for simu backend.
// Run: k6 run ops/load.k6.js
// Ramps a cohort of users through signup → me → list. Safe to run against dev.

import http from 'k6/http'
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

  const logout = http.post(`${BASE}/api/auth/logout`, null, { jar })
  check(logout, { 'logout 204': (r) => r.status === 204 })

  sleep(1)
}
