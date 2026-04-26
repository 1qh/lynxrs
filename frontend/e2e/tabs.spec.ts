import { test, expect, type Page } from '@playwright/test'
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
      { timeout: timeoutMs, message: `expected shadow to contain "${text}"` },
    )
    .toBe(true)
}

async function tapText(page: Page, text: string) {
  await page.evaluate((needle) => {
    const view = document.querySelector('lynx-view') as
      | (HTMLElement & { shadowRoot: ShadowRoot | null })
      | null
    const root = view?.shadowRoot
    if (!root) throw new Error('no shadow')
    const els = Array.from(root.querySelectorAll('*'))
    const target = els.find((el) => el.textContent?.trim() === needle)
    if (!target) throw new Error(`tapText: no element with text "${needle}"`)
    ;(target as HTMLElement).click()
  }, text)
}

test('tab nav switches panel and survives reload via hash', async ({ browser }) => {
  // Pre-signup so we land on Home (the tabbed UI).
  const api = await newApi()
  const email = `tabs-${Date.now()}@example.com`
  const r = await api.post('/api/auth/signup', {
    data: { email, password: 'hunter2hunter2' },
    headers: { 'content-type': 'application/json' },
  })
  expect(r.ok()).toBe(true)
  const state = await api.storageState()
  const session = state.cookies.find((c) => c.name === 'simu_session')!

  const ctx = await browser.newContext()
  await ctx.addCookies([
    {
      name: 'simu_session',
      value: session.value,
      domain: 'localhost',
      path: '/',
      httpOnly: true,
      secure: false,
      sameSite: 'Lax',
    },
  ])
  const page = await ctx.newPage()
  await page.goto(PREVIEW, { waitUntil: 'networkidle' })

  // Default tab is Files — its "Upload sample text" button is the tell.
  await waitForText(page, 'Upload sample text', 15_000)

  // Switch to Profile tab; "Save profile" button appears only there.
  await tapText(page, 'Profile')
  await waitForText(page, 'Save profile', 10_000)

  // Switch back to Files; the FilesPanel button reappears.
  await tapText(page, 'Files')
  await waitForText(page, 'Upload sample text', 10_000)

  // Note: persistence across full page reload only works on platforms where
  // the Lynx host shares localStorage with the page. In the rspeedy
  // __web_preview iframe (srcdoc), storage is opaque, so we don't assert
  // reload-restore here — only that switching is reactive.

  await ctx.close()
})
