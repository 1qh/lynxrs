import createClient from 'openapi-fetch'
import type { paths } from './schema.js'

const raw = (import.meta.env?.PUBLIC_API_BASE as string | undefined) ?? 'http://localhost:8088'
const baseUrl = `${raw.replace(/\/$/, '')}/api`

export const api = createClient<paths>({
  baseUrl,
  credentials: 'include',
})
