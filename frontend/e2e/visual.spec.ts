import { test, expect, type Page, request as pwRequest } from '@playwright/test'
import { newApi } from './_api'

const PREVIEW = '/__web_preview?casename=main.web.bundle'

async function waitForText(page: Page, text: string, timeoutMs = 20_000) {
  await expect
    .poll(
      async () =>
        page.evaluate((needle) => {
          const view = document.querySelector('lynx-view') as
            | (HTMLElement & { shadowRoot: ShadowRoot | null })
            | null
          return view?.shadowRoot?.textContent?.includes(needle) ?? false
        }, text),
      { timeout: timeoutMs },
    )
    .toBe(true)
}

test('visual regression: boot landing', async ({ page }) => {
  await page.goto(PREVIEW, { waitUntil: 'networkidle' })

  // Wait until Lynx shadow paints the "Create account" heading.
  await expect
    .poll(
      async () =>
        page.evaluate(() => {
          const view = document.querySelector('lynx-view') as
            | (HTMLElement & { shadowRoot: ShadowRoot | null })
            | null
          return view?.shadowRoot?.textContent?.includes('Create account') ?? false
        }),
      { timeout: 20_000 },
    )
    .toBe(true)

  // Wait a beat for any layout settle.
  await page.waitForTimeout(500)

  await expect(page).toHaveScreenshot('boot-landing.png', {
    fullPage: false,
    maxDiffPixelRatio: 0.02,
  })
})

test('visual regression: home after signup', async ({ browser }) => {
  // Pre-signup via HTTP so the Lynx app lands on Home on /me bootstrap.
  const api = await newApi()
  const email = `visual-home-${Date.now()}@example.com`
  const res = await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  expect(res.ok()).toBe(true)
  const state = await api.storageState()
  const session = state.cookies.find((c) => c.name === 'simu_session')!

  const ctx = await browser.newContext()
  await ctx.addCookies([
    { name: 'simu_session', value: session.value, domain: 'localhost', path: '/', httpOnly: true, secure: false, sameSite: 'Lax' },
  ])
  const page = await ctx.newPage()
  await page.goto(PREVIEW, { waitUntil: 'networkidle' })
  await waitForText(page, `Hello ${email}`, 20_000)
  await page.waitForTimeout(500)
  await expect(page).toHaveScreenshot('home-logged-in.png', {
    fullPage: false,
    // Email address varies per run → mask the H2 area; for spike we allow wider diff.
    maxDiffPixelRatio: 0.15,
  })
  await ctx.close()
})
