// Fixtures for e2e tests: the app on the in-memory mock, driven through CDP.
//
//   import { test, expect } from '../harness/app'
//   test('...', async ({ app }) => { await app.unlock(); await app.at('/'); ... })
//
// Waiting is by state (`waitFor`), never by sleeping. Animations are off.
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'

import { inject, test as base } from 'vitest'

import { attach, type Page } from './cdp.mjs'

export { expect } from 'vitest'

const ROOT = new URL('../../..', import.meta.url).pathname
export const SCREENSHOTS = join(ROOT, '.cache/e2e')

const VIEWPORTS = { 1280: [1280, 720, false], 390: [390, 844, true] } as const
export type Width = keyof typeof VIEWPORTS

const NO_MOTION = `(() => {
  const css = '*,*::before,*::after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;transition-delay:0s!important;scroll-behavior:auto!important}'
  const put = () => { const el = document.createElement('style'); el.textContent = css; document.documentElement.append(el) }
  if (document.documentElement) put(); else document.addEventListener('DOMContentLoaded', put)
})()`

const APP = `const app = document.querySelector('#app').__vue_app__; const gp = app.config.globalProperties;`

export interface App {
  page: Page
  /** Open the app and unlock the mock vault (password `secret`). Safe to call again. */
  unlock(options?: { locale?: 'ru' | 'en' }): Promise<void>
  /** Go to a route (after `unlock`) and wait until it is the current one and has drawn. */
  at(route: string, options?: { width?: Width; theme?: 'dark' | 'light' }): Promise<void>
  viewport(width: Width): Promise<void>
  /** Resolves when `code` (a function body run in the page) returns a truthy value; fails with `what` after `timeout` ms. */
  waitFor(code: string, what: string, timeout?: number): Promise<void>
  click(selector: string): Promise<void>
  exists(selector: string): Promise<boolean>
  /** Click the first element matching `selector` whose text matches the regular expression `pattern` (case-insensitive). */
  clickText(selector: string, pattern: string): Promise<void>
  /** Focus a field, replace its content with `text` the way a keyboard does (real input events). */
  type(selector: string, text: string): Promise<void>
  /** A key press on the focused element (`Enter`, `Escape`, `Tab`…). */
  press(key: string): Promise<void>
  /** Trimmed text of the first element matching `selector` (empty when there is none). */
  text(selector: string): Promise<string>
  /** `value` of the first matching field. */
  value(selector: string): Promise<string>
  screenshot(name: string): Promise<string>
}

function describeApp(page: Page, env: { url: string }): App {
  const waitFor: App['waitFor'] = async (code, what, timeout = 15000) => {
    const ok = await page.eval<boolean>(`
      const deadline = Date.now() + ${timeout}
      const check = async () => { ${code} }
      while (Date.now() < deadline) {
        if (await check()) return true
        await new Promise((done) => requestAnimationFrame(() => setTimeout(done, 30)))
      }
      return false`)
    if (!ok) throw new Error(`timed out after ${timeout} ms waiting for: ${what}`)
  }
  const mounted = () =>
    waitFor(`return Boolean(document.querySelector('#app')?.__vue_app__)`, 'the app to mount', 30000)

  const unlockOnce = async (locale: string) => {
    await mounted()
    await page.eval(`${APP}
      // Tours and What's new pop over the views: mark them all seen first.
      const { TOURS } = await import('/src/shared/lib/tour/tours.ts')
      const { WHATS_NEW } = await import('/src/shared/config/whats-new.ts')
      const settings = gp.$pinia._s.get('settings')
      await settings.loadApp()
      await settings.updateApp({ completed_tours: Object.keys(TOURS), seen_whats_new: WHATS_NEW.map((entry) => entry.id) })
      await gp.$pinia._s.get('vault').unlock('/tmp/mock.wallet', 'secret')
      const { setLocalePreference } = await import('/src/shared/lib/locale.ts')
      await setLocalePreference('${locale}')
      await gp.$router.push('/')
      return true`)
    await waitFor(
      `${APP} return gp.$route.path === '/' && document.querySelector('[data-tour="quick-add"]') !== null`,
      'the home screen after unlock',
    )
  }

  const isUnlocked = () =>
    page
      .eval<boolean>(
        `const root = document.querySelector('#app')?.__vue_app__; return Boolean(root) && root.config.globalProperties.$pinia._s.get('vault')?.status?.unlocked === true`,
      )
      .catch(() => false)

  const unlock: App['unlock'] = async ({ locale = 'en' } = {}) => {
    if (await isUnlocked()) return
    await page.goto(env.url)
    for (let attempt = 1; ; attempt++) {
      try {
        await unlockOnce(locale)
        return
      } catch (error) {
        // A cold Vite start pre-bundles dependencies and reloads the page once.
        if (attempt === 3) throw error
      }
    }
  }

  const viewport: App['viewport'] = async (width) => {
    const [viewWidth, viewHeight, touch] = VIEWPORTS[width]
    await page.viewport(viewWidth, viewHeight, touch)
  }

  return {
    page,
    unlock,
    viewport,
    waitFor,
    async at(route, { width = 1280, theme } = {}) {
      if (!(await isUnlocked())) await unlock()
      await viewport(width)
      if (theme) {
        await page.eval(`${APP} await gp.$pinia._s.get('settings').updateApp({ theme: '${theme}' }); return true`)
      }
      await page.eval(`${APP} await gp.$router.push(${JSON.stringify(route)}); return true`)
      const path = route.split('?')[0]
      await waitFor(`${APP} return gp.$route.path === ${JSON.stringify(path)}`, `route ${route}`)
      await page.eval(
        `await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))); return true`,
      )
    },
    async click(selector) {
      await waitFor(`return document.querySelector(${JSON.stringify(selector)}) !== null`, `${selector} to exist`)
      await page.eval(`document.querySelector(${JSON.stringify(selector)}).click(); return true`)
    },
    async clickText(selector, pattern) {
      const find = `[...document.querySelectorAll(${JSON.stringify(selector)})].find((el) => new RegExp(${JSON.stringify(pattern)}, 'i').test(el.textContent.trim()))`
      await waitFor(`return Boolean(${find})`, `${selector} with text /${pattern}/ to exist`)
      await page.eval(`${find}.click(); return true`)
    },
    async type(selector, text) {
      await waitFor(`return document.querySelector(${JSON.stringify(selector)}) !== null`, `${selector} to exist`)
      await page.eval(
        `const el = document.querySelector(${JSON.stringify(selector)}); el.focus(); el.select?.(); return true`,
      )
      await page.send('Input.dispatchKeyEvent', {
        type: 'keyDown',
        key: 'Delete',
        code: 'Delete',
        windowsVirtualKeyCode: 46,
      })
      await page.send('Input.dispatchKeyEvent', {
        type: 'keyUp',
        key: 'Delete',
        code: 'Delete',
        windowsVirtualKeyCode: 46,
      })
      await page.send('Input.insertText', { text })
    },
    async press(key) {
      const codes: Record<string, number> = { Enter: 13, Escape: 27, Tab: 9 }
      const params = { key, code: key, windowsVirtualKeyCode: codes[key] ?? 0 }
      await page.send('Input.dispatchKeyEvent', { type: 'keyDown', ...params })
      await page.send('Input.dispatchKeyEvent', { type: 'keyUp', ...params })
    },
    text: (selector) =>
      page.eval<string>(
        `return (document.querySelector(${JSON.stringify(selector)})?.textContent ?? '').replace(/\\s+/g, ' ').trim()`,
      ),
    value: (selector) => page.eval<string>(`return document.querySelector(${JSON.stringify(selector)})?.value ?? ''`),
    exists: (selector) => page.eval<boolean>(`return document.querySelector(${JSON.stringify(selector)}) !== null`),
    async screenshot(name) {
      mkdirSync(SCREENSHOTS, { recursive: true })
      const path = join(SCREENSHOTS, `${name}.png`)
      await page.shot(path)
      return path
    },
  }
}

export interface ConnectOptions {
  /** Switch CSS animations and transitions off (default). Layout checks measure motion and keep them. */
  animations?: boolean
}

/**
 * Attach to the shared browser. The connection holds the emulation (viewport, touch), so a file that
 * needs it across tests (layout, links) connects once; the `app` fixture connects per test.
 */
export async function connect({ animations = false }: ConnectOptions = {}): Promise<App & { close(): Promise<void> }> {
  const env = inject('e2e')
  const page = await attach(env.debugPort)
  let identifier: string | null = null
  if (!animations) {
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] })
    const { result } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: NO_MOTION })
    identifier = result.identifier
    // The page already loaded in an earlier test keeps its style: put it in there too.
    await page.eval(`${NO_MOTION}; return true`).catch(() => {})
  }
  const app = describeApp(page, env)
  return {
    ...app,
    async close() {
      if (identifier) {
        await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }).catch(() => {})
        await page.send('Emulation.setEmulatedMedia', { features: [] }).catch(() => {})
      }
      await page.close()
    },
  }
}

export const test = base.extend<{ app: App }>({
  app: async ({ task }, use) => {
    const app = await connect()
    try {
      await use(app)
      // The test body has finished; its failure (if any) is on the task already.
      if (task.result?.state === 'fail' || task.result?.errors?.length) {
        const file = task.name.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '')
        const path = await app.screenshot(`FAILED-${file}`).catch(() => null)
        if (path) console.error(`e2e: screenshot of the failure: ${path}`)
      }
    } finally {
      await app.close()
    }
  },
})
