// What a shot does before the picture, the canvas frame and the side-by-side pair.
//
// A shot: { name, route, steps?, scenarios?, phone?, desktop?, canvas?: { board, frame } }.
// A step is one of:
//   { click: 'css' }  or  { click: { text: 'Сохранить', within?: 'css' } }
//   { hover: 'css' }   { fill: ['css', 'text'] }   { press: 'ctrl+k' | 'Escape' | … }
//   { type: ['css', 'text'] } — real key input, for a combobox that keeps its own search term
//   { wait: ms }       { eval: 'page code, returns anything' }
// `login: false` — no login: the shot starts from a reloaded, locked app (mock state and LocalStorage reset), route as given.
// `desktop` / `phone` steps run only there (`{ desktop: [...] }`, `{ phone: [...] }`).
import { pathToFileURL } from 'node:url'

import { login, mounted, setTheme } from '../check-layout/scenario.mjs'

/** Page code: wait for the app to mount (a Vite reload may be under way), then `gp` is its global properties. */
export const GP = `
  for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
  if (!document.querySelector('#app')?.__vue_app__) throw new Error('the app did not mount in 30 s')
  const gp = document.querySelector('#app').__vue_app__.config.globalProperties;`
const APP = GP

/** Find by CSS or by visible text; the element, or a thrown error naming what is missing. */
const FIND = `
  const find = (what) => {
    if (typeof what === 'string') {
      const el = [...document.querySelectorAll(what)].find((node) => node.getClientRects().length)
      if (!el) throw new Error('no element ' + what)
      return el
    }
    const scope = what.within ? document.querySelector(what.within) : document
    const el = [...(scope ?? document).querySelectorAll('button, a, [role="menuitem"], [role="tab"], [role="option"], label, [role="button"]')]
      .find((node) => node.textContent.trim().startsWith(what.text) && node.getClientRects().length)
    if (!el) throw new Error('no element with text ' + what.text)
    return el
  }`

/** A reloaded app, locked, in `theme`: the start of a `login: false` shot. */
export async function freshStart(page, url, theme, touch) {
  await page.eval(`try { localStorage.clear(); sessionStorage.clear() } catch {} return true`)
  await page.goto(url)
  await mounted(page)
  await page.eval(`document.documentElement.style.cssText = '${touch ? '--native-inset-top:47px;--native-inset-bottom:34px' : ''}'; return true`)
  await page.eval(`const { setLocalePreference } = await import('/src/shared/lib/locale.ts'); await setLocalePreference('ru'); return true`)
  await setTheme(page, theme)
}

/** Close what an earlier shot left open, then open the route. */
async function reset(page, route) {
  for (let round = 0; round < 3; round++) {
    await page.eval(`document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true`)
    await page.sleep(80)
  }
  const target = route.startsWith('settings:') ? `{ path: '/', query: { settings: '${route.slice(9)}' } }` : `'${route}'`
  // The same route again would keep the page and whatever the last shot picked on it.
  await page.eval(`${APP} if (gp.$route.path !== '/') await gp.$router.push('/'); await gp.$router.push(${target}); return true`)
  await page.sleep(500)
  await page.mouse(2, 2)
}

function keyEvent(combo) {
  const parts = combo.split('+')
  const key = parts.pop()
  const code = key.length === 1 ? `Key${key.toUpperCase()}` : key
  const mods = { ctrlKey: parts.includes('ctrl'), shiftKey: parts.includes('shift'), altKey: parts.includes('alt') }
  return JSON.stringify({ key: key.length === 1 ? key : key, code, bubbles: true, cancelable: true, ...mods })
}

async function step(page, item, touch) {
  if (item.desktop) return touch ? undefined : runList(page, item.desktop, touch)
  if (item.phone) return touch ? runList(page, item.phone, touch) : undefined
  if (item.wait) return page.sleep(item.wait)
  if (item.eval) return page.eval(item.eval)
  if (item.press) {
    await page.eval(`(document.activeElement ?? document).dispatchEvent(new KeyboardEvent('keydown', ${keyEvent(item.press)})); return true`)
    return page.sleep(350)
  }
  if (item.type) {
    const [what, text] = item.type
    await page.eval(`${FIND}
      find(${JSON.stringify(what)}).focus()
      return true`)
    await page.send('Input.insertText', { text })
    return page.sleep(200)
  }
  if (item.fill) {
    const [what, text] = item.fill
    await page.eval(`${FIND}
      const el = find(${JSON.stringify(what)})
      el.focus()
      const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value').set
      setter.call(el, ${JSON.stringify(text)})
      el.dispatchEvent(new Event('input', { bubbles: true }))
      el.dispatchEvent(new Event('change', { bubbles: true }))
      return true`)
    return page.sleep(200)
  }
  const what = item.click ?? item.hover
  const at = await page.eval(`${FIND}
    const el = find(${JSON.stringify(what)})
    el.scrollIntoView({ block: 'center' })
    const rect = el.getBoundingClientRect()
    return [Math.round(rect.left + rect.width / 2), Math.round(rect.top + rect.height / 2)]`)
  await page.mouse(at[0], at[1])
  if (item.click) {
    for (const type of ['mousePressed', 'mouseReleased']) {
      await page.send('Input.dispatchMouseEvent', { type, x: at[0], y: at[1], button: 'left', clickCount: 1 })
    }
  }
  return page.sleep(item.click ? 450 : 350)
}

async function runList(page, list, touch) {
  for (const item of list) await step(page, item, touch)
}

/**
 * A shared-login shot needs the mounted, logged-in app: Vite may have reloaded the page since
 * the last shot, and a reload leaves the unlock screen. Logs in again and restores the theme.
 */
export async function ensureReady(page, theme, touch) {
  await mounted(page)
  const locked = await page.eval(`${APP} return gp.$pinia._s.get('vault').locked !== false`)
  if (!locked) return
  await login(page)
  await page.eval(`document.documentElement.style.cssText = '${touch ? '--native-inset-top:47px;--native-inset-bottom:34px' : ''}'; return true`)
  await setTheme(page, theme)
}

export async function runSteps(page, shot, touch) {
  await mounted(page)
  await reset(page, shot.route)
  await runList(page, shot.steps ?? [], touch)
}

/** Let data land and animations end; close tours that popped up. */
export async function settle(page, scenario) {
  // A failed query retries three times (1 + 2 + 4 s) before the error shows.
  await page.sleep(scenario === 'error' ? 7600 : 600)
  await page.eval(`document.querySelector('.driver-popover-close-btn')?.click(); return true`)
  await page.sleep(150)
}

/**
 * One frame of the canvas: board file, the frame whose heading starts with `frame`
 * (e.g. '1.'), the theme column, the desktop (`d`) or phone (`p`) screen.
 */
export async function canvasShot(page, dir, { board, frame }, theme, kind, file) {
  const width = kind === 'd' ? 1300 : 420
  await page.viewport(width, 900, false)
  await page.goto(pathToFileURL(`${dir}/Full-${board}.dc.html.html`).href)
  const rect = await page.eval(`
    const section = [...document.querySelectorAll('section')]
      .find((node) => node.querySelector('h3')?.textContent.trim().startsWith(${JSON.stringify(frame)}))
    const screen = section?.querySelector('.t-${theme} .scr.${kind}')
    if (!screen) return null
    screen.scrollIntoView()
    const box = screen.getBoundingClientRect()
    return { x: box.left + scrollX, y: box.top + scrollY, width: box.width, height: box.height }`)
  if (!rect) return false
  const res = await page.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
    clip: { ...rect, scale: 1 },
  })
  const { writeFileSync } = await import('node:fs')
  writeFileSync(file, Buffer.from(res.result.data, 'base64'))
  return true
}

/** Canvas | app, side by side, labelled. */
export async function pairShot(page, canvas, app, file) {
  const html = `<!doctype html><meta charset="utf-8"><body style="margin:0;background:#888;font:13px sans-serif">
    <div style="display:flex;gap:16px;padding:8px;align-items:flex-start">
      <figure style="margin:0"><figcaption>холст</figcaption><img id="a" src="${pathToFileURL(canvas).href}"></figure>
      <figure style="margin:0"><figcaption>приложение</figcaption><img id="b" src="${pathToFileURL(app).href}"></figure>
    </div></body>`
  // A file page: a data: URL may not load file: images.
  const { writeFileSync } = await import('node:fs')
  const htmlFile = file.replace(/\.png$/, '.html')
  writeFileSync(htmlFile, html)
  await page.goto(pathToFileURL(htmlFile).href)
  const size = await page.eval(`
    await Promise.all([...document.images].map((img) => img.decode().catch(() => {})))
    return [document.body.scrollWidth, document.body.scrollHeight]`)
  await page.viewport(size[0], size[1], false)
  await page.shot(file)
}
