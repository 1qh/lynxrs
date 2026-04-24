import { test, expect, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'
import * as OTPAuth from 'otpauth'

const BACKEND = 'http://localhost:8088'

test('MFA recovery code lets you log in without TOTP, and is single-use', async () => {
  const api = await newApi()
  const email = `mfa-rec-${Date.now()}@t.local`
  const password = 'hunter2hunter2'
  await api.post('/api/auth/signup', {
    data: { email, password },
    headers: { 'content-type': 'application/json' },
  })
  const { secret } = (await (await api.post('/api/mfa/enroll')).json()) as { secret: string }
  const totp = new OTPAuth.TOTP({
    issuer: 'Simu',
    label: email,
    algorithm: 'SHA1',
    digits: 6,
    period: 30,
    secret: OTPAuth.Secret.fromBase32(secret),
  })
  await api.post('/api/mfa/activate', {
    data: { code: totp.generate() },
    headers: { 'content-type': 'application/json' },
  })

  const recRes = await api.post('/api/mfa/recovery-codes')
  expect(recRes.status()).toBe(200)
  const { codes } = (await recRes.json()) as { codes: string[] }
  expect(codes).toHaveLength(10)

  // Log in with a recovery code.
  const anon = await newApi()
  const login1 = await anon.post('/api/auth/login', {
    data: { email, password, totp_code: codes[0] },
    headers: { 'content-type': 'application/json' },
  })
  expect(login1.status()).toBe(200)

  // Same code a second time → rejected.
  const anon2 = await newApi()
  const login2 = await anon2.post('/api/auth/login', {
    data: { email, password, totp_code: codes[0] },
    headers: { 'content-type': 'application/json' },
  })
  expect(login2.status()).toBe(401)
})
