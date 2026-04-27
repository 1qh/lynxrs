import { test, expect, type Page } from '@playwright/test'

const PREVIEW = '/__web_preview?casename=main.web.bundle'

async function waitForText(page: Page, text: string, timeoutMs = 20_000) {
  // Probe shadow DOM textContent until it contains `text`.
  await expect
    .poll(
      async () => {
        return page.evaluate((needle) => {
          const view = document.querySelector('lynx-view') as
            | (HTMLElement & { shadowRoot: ShadowRoot | null })
            | null
          return view?.shadowRoot?.textContent?.includes(needle) ?? false
        }, text)
      },
      { timeout: timeoutMs, message: `expected shadow to contain "${text}"` },
    )
    .toBe(true)
}

async function fillInput(page: Page, placeholder: string, value: string) {
  // Lynx renders <x-input placeholder=..>. Its shadow root contains a real <input>.
  // Reach into the double-shadow to set value + fire input event matching Lynx bindinput.
  await page.evaluate(
    ({ placeholder, value }) => {
      const view = document.querySelector('lynx-view') as
        | (HTMLElement & { shadowRoot: ShadowRoot | null })
        | null
      const xInput = view?.shadowRoot?.querySelector(
        `x-input[placeholder="${placeholder}"]`,
      ) as (HTMLElement & { shadowRoot: ShadowRoot | null }) | null
      if (!xInput) throw new Error(`x-input[placeholder=${placeholder}] not found`)
      const real = xInput.shadowRoot?.querySelector('input') as HTMLInputElement | null
      if (!real) throw new Error('inner <input> not found')
      real.focus()
      real.value = value
      real.dispatchEvent(new Event('input', { bubbles: true, composed: true }))
      real.dispatchEvent(new Event('change', { bubbles: true, composed: true }))
      real.blur()
    },
    { placeholder, value },
  )
}

async function tapText(page: Page, text: string) {
  await page.evaluate((needle) => {
    const view = document.querySelector('lynx-view') as
      | (HTMLElement & { shadowRoot: ShadowRoot | null })
      | null
    const all = view?.shadowRoot?.querySelectorAll('x-text, x-view, raw-text') ?? []
    for (const el of Array.from(all)) {
      if ((el.textContent ?? '').trim() === needle.trim()) {
        ;(el as HTMLElement).click()
        return
      }
    }
    throw new Error(`tapText: no element with text "${needle}"`)
  }, text)
}

test.beforeEach(async ({ page, request }) => {
  await page.context().clearCookies()
  try {
    await request.post('http://localhost:8088/api/auth/logout')
  } catch {}
  page.on('console', (msg) => console.log(`[browser ${msg.type()}]`, msg.text()))
  page.on('pageerror', (err) => console.log(`[browser error]`, err.message))
  page.on('requestfailed', (req) =>
    console.log(`[req-fail]`, req.method(), req.url(), req.failure()?.errorText),
  )
})

test('boot → signup → upload → logout → relogin', async ({ page }) => {
  await page.goto(PREVIEW)
  await expect(page.locator('lynx-view')).toBeVisible()

  await waitForText(page, 'Create account', 30_000)

  const email = `pw-${Date.now()}@example.com`
  const password = 'hunter2hunter2'

  await fillInput(page, 'email', email)
  await fillInput(page, 'password', password)
  await tapText(page, 'Sign up')

  await waitForText(page, `Hello ${email}`, 15_000)

  // Default tab is now Chat — switch to Files for the upload assertion.
  await tapText(page, 'Files')
  await waitForText(page, 'Upload sample text', 10_000)
  await tapText(page, 'Upload sample text')
  await waitForText(page, 'note-', 10_000)

  // Logout lives behind the More menu in the new mobile shell.
  await tapText(page, 'More')
  await waitForText(page, 'Log out', 5_000)
  await tapText(page, 'Log out')
  await waitForText(page, 'Create account', 10_000)

  // Re-login
  await tapText(page, 'Have an account? Log in')
  await waitForText(page, 'Login', 5_000)
  await fillInput(page, 'email', email)
  await fillInput(page, 'password', password)
  await tapText(page, 'Log in')

  await waitForText(page, `Hello ${email}`, 15_000)
  await tapText(page, 'Files')
  await waitForText(page, 'note-', 10_000)
})
