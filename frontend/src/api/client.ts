import createClient from 'openapi-fetch'
import type { paths } from './schema.js'

const raw = (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'
const baseUrl = `${raw.replace(/\/$/, '')}/api`

function readCookie(name: string): string | undefined {
  if (typeof document === 'undefined') return undefined
  const m = document.cookie.match(new RegExp('(?:^|; )' + name + '=([^;]*)'))
  return m ? decodeURIComponent(m[1]!) : undefined
}

// Cross-origin fallback: the server returns csrf_token in login/signup/me bodies,
// and we cache it here. Same-origin setups can alternatively read the cookie.
let csrfMemory: string | undefined
export function rememberCsrf(token: string | undefined) {
  if (token) csrfMemory = token
}

export const api = createClient<paths>({
  baseUrl,
  credentials: 'include',
})

api.use({
  async onRequest({ request }) {
    const method = request.method.toUpperCase()
    if (method === 'GET' || method === 'HEAD' || method === 'OPTIONS') return
    const csrf = readCookie('simu_csrf') ?? csrfMemory
    if (csrf) request.headers.set('x-csrf-token', csrf)
  },
  async onResponse({ response }) {
    try {
      const ct = response.headers.get('content-type') ?? ''
      if (!ct.includes('json')) return
      const clone = response.clone()
      const body = await clone.json()
      if (body && typeof body === 'object' && typeof body.csrf_token === 'string') {
        csrfMemory = body.csrf_token
      }
    } catch {}
  },
})
