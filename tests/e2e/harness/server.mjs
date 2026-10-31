// Shared by the browser gates (e2e, check-layout, check-links): a free port and a Vite dev server on it.
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'

const root = new URL('../../..', import.meta.url).pathname
const HOST = '127.0.0.1'
const STARTUP_LIMIT_MS = 180_000

export const freePort = () =>
  new Promise((done) => {
    const server = createServer()
    server.listen(0, () => {
      const { port } = server.address()
      server.close(() => done(port))
    })
  })

export async function startVite() {
  const port = await freePort()
  // `localhost` may resolve to ::1 for Vite and to 127.0.0.1 for fetch, a container decides.
  const args = ['--host', HOST, '--port', String(port), '--strictPort', '--clearScreen', 'false']
  const vite = spawn('pnpm', ['exec', 'vite', ...args], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let output = ''
  const keep = (chunk) => {
    output = (output + chunk).slice(-2000)
  }
  vite.stdout.on('data', keep)
  vite.stderr.on('data', keep)
  const url = `http://${HOST}:${port}/`
  let lastFailure = 'no attempt'
  const deadline = Date.now() + STARTUP_LIMIT_MS
  while (Date.now() < deadline) {
    try {
      const signal = AbortSignal.timeout(deadline - Date.now())
      const response = await fetch(url, { signal })
      if (response.ok) return { vite, url }
      lastFailure = `HTTP ${response.status}`
    } catch (error) {
      lastFailure = String(error.cause ?? error)
    }
    await new Promise((done) => setTimeout(done, 150))
  }
  vite.kill()
  throw new Error(`vite did not start (${lastFailure}):\n${output}`)
}
