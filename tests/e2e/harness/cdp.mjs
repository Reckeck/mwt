// A minimal Chrome DevTools Protocol driver: headless Chromium, one page,
// evaluate, mouse, viewport and media emulation. No dependencies.
// `launch` starts a browser and drives it (check-layout, check-links); the vitest e2e
// project starts it once in globalSetup (`spawnBrowser`) and every test file `attach`es.
import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const sleep = (ms) => new Promise((done) => setTimeout(done, ms))

const CANDIDATES = ['/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome', '/usr/bin/google-chrome-stable']

export function findChromium() {
  const wanted = process.env.CHROMIUM
  if (wanted) return wanted
  return CANDIDATES.find((path) => existsSync(path)) ?? null
}

/** Start headless Chromium with remote debugging on `port`; resolves once its page target answers. */
export async function spawnBrowser(binary, port) {
  const profile = mkdtempSync(join(tmpdir(), 'check-layout-'))
  const browser = spawn(
    binary,
    [
      '--headless=new',
      `--remote-debugging-port=${port}`,
      '--no-first-run',
      '--no-sandbox',
      '--disable-gpu',
      '--hide-scrollbars=false',
      // The app window is WebKitGTK with a mouse: pointer fine and hover (headless reports none).
      // Touch emulation below switches a phone view to coarse / no hover.
      '--blink-settings=primaryPointerType=4,availablePointerTypes=4,primaryHoverType=2,availableHoverTypes=2',
      `--user-data-dir=${profile}`,
      'about:blank',
    ],
    { stdio: 'ignore' },
  )
  let targets = null
  for (let tries = 0; tries < 100 && !targets; tries++) {
    try {
      targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json()
    } catch {
      await sleep(150)
    }
  }
  if (!targets) {
    browser.kill()
    throw new Error('chromium did not start')
  }
  const stop = async () => {
    const exited = new Promise((done) => browser.once('exit', done))
    browser.kill()
    await Promise.race([exited, sleep(3000)])
    try {
      rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 })
    } catch {
      // A leftover temp profile is harmless.
    }
  }
  return { stop }
}

/** Connect to the first page of a running browser. `close()` only drops the connection. */
export async function attach(port) {
  const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json()
  const target = targets.find((item) => item.type === 'page')
  const ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((done, fail) => {
    ws.addEventListener('open', done)
    ws.addEventListener('error', fail)
  })
  let seq = 0
  const pending = new Map()
  ws.addEventListener('message', (event) => {
    const msg = JSON.parse(event.data)
    if (msg.id && pending.has(msg.id)) {
      pending.get(msg.id)(msg)
      pending.delete(msg.id)
    }
    // «Leave the page?» from an unsaved editor would block the next navigation forever.
    if (msg.method === 'Page.javascriptDialogOpening') {
      ws.send(JSON.stringify({ id: ++seq, method: 'Page.handleJavaScriptDialog', params: { accept: true } }))
    }
  })
  const send = (method, params = {}) =>
    new Promise((done) => {
      const id = ++seq
      pending.set(id, done)
      ws.send(JSON.stringify({ id, method, params }))
    })
  await send('Page.enable')
  await send('Runtime.enable')

  return {
    send,
    sleep,
    /** Run `code` (an async function body) in the page and return its JSON value; throws on a page error. */
    async eval(code) {
      const res = await send('Runtime.evaluate', {
        expression: `(async () => { ${code} })()`,
        awaitPromise: true,
        returnByValue: true,
      })
      const failure = res.result?.exceptionDetails
      if (failure) throw new Error(failure.exception?.description ?? failure.text)
      return res.result?.result?.value
    },
    async goto(url) {
      await send('Page.navigate', { url })
      await sleep(1500)
    },
    /** Touch emulation cannot be turned back into a fine pointer: run every mouse view first. */
    async viewport(width, height, touch) {
      await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: touch })
      await send('Emulation.setTouchEmulationEnabled', { enabled: touch, maxTouchPoints: touch ? 5 : 0 })
      await sleep(300)
    },
    async mouse(x, y) {
      await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y })
    },
    async shot(path) {
      const res = await send('Page.captureScreenshot', { format: 'png' })
      writeFileSync(path, Buffer.from(res.result.data, 'base64'))
    },
    async close() {
      ws.close()
    },
  }
}

export async function launch(binary, port) {
  const browser = await spawnBrowser(binary, port)
  const page = await attach(port)
  const detach = page.close
  page.close = async () => {
    await detach()
    await browser.stop()
  }
  return page
}
