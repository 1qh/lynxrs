import { test, expect } from '@playwright/test'
import { newApi } from './_api'
import { execSync } from 'node:child_process'

// Restart backend with a low per-user RPS so we can actually observe the limit.
// If we can't spawn a backend with the right env, skip.
test('per-user rate limit returns 429 rate_limited', async () => {
  // The dev backend on :8088 is started with PER_USER_RPS=500. Spawn a second
  // backend on :8089 with PER_USER_RPS=2 specifically for this test.
  const port = 8089
  // Read dev .env to inherit DATABASE_URL, SESSION_SECRET, S3_*, etc.
  const envText = execSync('cat /Users/o/simu/backend/.env').toString()
  const baseEnv: Record<string, string> = { ...(process.env as Record<string, string>) }
  for (const line of envText.split('\n')) {
    const m = line.match(/^([A-Z][A-Z0-9_]*)=(.*)$/)
    if (m) baseEnv[m[1]!] = m[2]!
  }
  const env = {
    ...baseEnv,
    BIND_ADDR: `127.0.0.1:${port}`,
    PER_USER_RPS: '2',
    RATE_LIMIT_RPS: '1000',
    RATE_LIMIT_BURST: '2000',
    PATH: '/opt/homebrew/opt/libpq/bin:' + (baseEnv.PATH ?? ''),
  }

  // Spawn backend.
  const bin = '/Users/o/simu/backend/target/release/simu-backend'
  const { spawn } = await import('node:child_process')
  const proc = spawn(bin, [], { env, stdio: 'pipe', detached: false })
  let stderr = ''
  proc.stderr?.on('data', (d) => (stderr += d.toString()))
  // Poll /health until the spawned backend is ready.
  const started = Date.now()
  let ready = false
  while (Date.now() - started < 15_000) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/health`)
      if (r.ok) { ready = true; break }
    } catch {}
    await new Promise((r) => setTimeout(r, 200))
  }
  if (!ready) {
    console.log('spawned backend stderr:', stderr.slice(-1200))
    proc.kill('SIGTERM')
    throw new Error('spawned rate-limit backend on 127.0.0.1:' + port + ' did not become ready in 15s')
  }

  try {
    const api = await newApi(`http://127.0.0.1:${port}`)
    const signup = await api.post('/api/auth/signup', {
      data: { email: `rl-${Date.now()}@t.local`, password: 'hunter2hunter2' },
      headers: { 'content-type': 'application/json' },
    })
    expect(signup.status()).toBe(201)

    // Fire 6 mutating requests; with PER_USER_RPS=2 the first ~2 pass, rest 429.
    const statuses: number[] = []
    for (let i = 0; i < 6; i++) {
      const r = await api.post('/api/files/json', {
        data: { filename: `x${i}.txt`, content_type: 'text/plain', data_base64: 'aGk=' },
        headers: { 'content-type': 'application/json' },
      })
      statuses.push(r.status())
    }
    const limited = statuses.filter((s) => s === 429)
    expect(limited.length).toBeGreaterThan(0)

    // Verify 429 body has code=rate_limited.
    const last = await api.post('/api/files/json', {
      data: { filename: 'last.txt', content_type: 'text/plain', data_base64: 'aGk=' },
      headers: { 'content-type': 'application/json' },
    })
    if (last.status() === 429) {
      const body = (await last.json()) as { code: string }
      expect(body.code).toBe('rate_limited')
    }
  } finally {
    proc.kill('SIGTERM')
  }
})
