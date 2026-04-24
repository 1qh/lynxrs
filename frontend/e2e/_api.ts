// Wraps Playwright's APIRequestContext so mutating requests automatically
// echo the simu_csrf cookie into an X-CSRF-Token header. Use `newApi()`
// anywhere you'd have called `pwRequest.newContext({ baseURL })`.
import { request as pwRequest, APIRequestContext, APIResponse } from '@playwright/test'

const BACKEND = 'http://localhost:8088'
const MUTATING = new Set(['POST', 'PUT', 'PATCH', 'DELETE'])

export interface Api {
  get: (url: string, opts?: Parameters<APIRequestContext['get']>[1]) => Promise<APIResponse>
  post: (url: string, opts?: Parameters<APIRequestContext['post']>[1]) => Promise<APIResponse>
  put: (url: string, opts?: Parameters<APIRequestContext['put']>[1]) => Promise<APIResponse>
  patch: (url: string, opts?: Parameters<APIRequestContext['patch']>[1]) => Promise<APIResponse>
  delete: (url: string, opts?: Parameters<APIRequestContext['delete']>[1]) => Promise<APIResponse>
  fetch: APIRequestContext['fetch']
  storageState: APIRequestContext['storageState']
  raw: APIRequestContext
}

async function csrfHeader(ctx: APIRequestContext): Promise<Record<string, string>> {
  const state = await ctx.storageState()
  const c = state.cookies.find((k) => k.name === 'simu_csrf')
  return c ? { 'x-csrf-token': c.value } : {}
}

function withCsrfHeaders(
  existing: Record<string, string> | undefined,
  csrf: Record<string, string>,
): Record<string, string> {
  return { ...(existing ?? {}), ...csrf }
}

export async function newApi(baseURL = BACKEND): Promise<Api> {
  const ctx = await pwRequest.newContext({ baseURL })
  const wrap = async <K extends 'get' | 'post' | 'put' | 'patch' | 'delete'>(
    method: K,
    url: string,
    opts?: Parameters<APIRequestContext[K]>[1],
  ): Promise<APIResponse> => {
    const needsCsrf = MUTATING.has(method.toUpperCase())
    const csrf = needsCsrf ? await csrfHeader(ctx) : {}
    const merged = {
      ...((opts ?? {}) as Record<string, unknown>),
      headers: withCsrfHeaders((opts as any)?.headers, csrf),
    }
    // @ts-expect-error — opts shape matches per-method
    return ctx[method](url, merged)
  }
  return {
    get: (u, o) => wrap('get', u, o),
    post: (u, o) => wrap('post', u, o),
    put: (u, o) => wrap('put', u, o),
    patch: (u, o) => wrap('patch', u, o),
    delete: (u, o) => wrap('delete', u, o),
    fetch: ctx.fetch.bind(ctx),
    storageState: ctx.storageState.bind(ctx),
    raw: ctx,
  }
}
