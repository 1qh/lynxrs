import { test, expect } from '@playwright/test'
import { newApi } from './_api'
import { execSync } from 'node:child_process'

test('per-user rate limit returns 429 rate_limited', async () => {
  // Precondition: simu-postgres + .env must be present.
  try {
    execSync('test -f /Users/o/simu/backend/.env', { stdio: 'pipe' })
    execSync('docker ps --format "{{.Names}}" | grep -q simu-postgres', { stdio: 'pipe' })
  } catch {
    test.skip(true, 'dev infra not up (need .env + simu-postgres container)')
    return
  }

  const port = 8089
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

  const bin = '/Users/o/simu/backend/target/release/simu-backend'
  const { spawn } = await import('node:child_process')
  const proc = spawn(bin, [], { env, stdio: 'pipe', detached: false })
  let stderr = ''
  proc.stderr?.on('data', (d) => (stderr += d.toString()))

  try {
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
      throw new Error(
        'spawned backend on 127.0.0.1:' + port + ' not ready in 15s — stderr: ' + stderr.slice(-1200),
      )
    }

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
    expect(statuses.filter((s) => s === 429).length).toBeGreaterThan(0)

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
    // Bulletproof cleanup: always kill the spawned process on any exit path.
    try {
      proc.kill('SIGTERM')
      await new Promise<void>((r) => {
        const t = setTimeout(() => { try { proc.kill('SIGKILL') } catch {}; r() }, 2000)
        proc.on('exit', () => { clearTimeout(t); r() })
      })
    } catch {}
  }
})
