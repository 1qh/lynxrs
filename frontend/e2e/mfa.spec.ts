import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import * as OTPAuth from 'otpauth'

const BACKEND = 'http://localhost:8088'

test('MFA enroll → activate → login requires code → disable', async () => {
  const api = await newApi()
  const email = `mfa-${Date.now()}@t.local`
  const password = 'hunter2hunter2'
  await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })

  const enrollRes = await api.post('/api/mfa/enroll')
  expect(enrollRes.status()).toBe(200)
  const enroll = (await enrollRes.json()) as { secret: string; otpauth_url: string }
  expect(enroll.secret).toBeTruthy()
  expect(enroll.otpauth_url).toContain('otpauth://totp/')

  const totp = new OTPAuth.TOTP({
    issuer: 'Simu',
    label: email,
    algorithm: 'SHA1',
    digits: 6,
    period: 30,
    secret: OTPAuth.Secret.fromBase32(enroll.secret),
  })

  const activate = await api.post('/api/mfa/activate', {
    data: { code: totp.generate() },
    headers: { 'content-type': 'application/json' },
  })
  expect(activate.status()).toBe(204)

  // Login without code → 401
  const anon1 = await newApi()
  const bad = await anon1.post('/api/auth/login', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  expect(bad.status()).toBe(401)

  // Login with code → 200
  const anon2 = await newApi()
  const good = await anon2.post('/api/auth/login', {
    data: { email, password, totp_code: totp.generate() },
    headers: { 'content-type': 'application/json' },
  })
  expect(good.status()).toBe(200)
  const body = (await good.json()) as { totp_enabled: boolean }
  expect(body.totp_enabled).toBe(true)

  // Disable
  const disable = await api.post('/api/mfa/disable', {
    data: { code: totp.generate() },
    headers: { 'content-type': 'application/json' },
  })
  expect(disable.status()).toBe(204)
})
