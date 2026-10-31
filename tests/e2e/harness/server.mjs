// Shared by the browser gates (e2e, check-layout, check-links): a free port and a Vite dev server on it.
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'

const root = new URL('../../..', import.meta.url).pathname
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
  const vite = spawn('pnpm', ['exec', 'vite', '--port', String(port), '--strictPort', '--clearScreen', 'false'], {
    cwd: root,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let output = ''
  const keep = (chunk) => {
    output = (output + chunk).slice(-2000)
  }
  vite.stdout.on('data', keep)
  vite.stderr.on('data', keep)
  const url = `http://localhost:${port}/`
  // A clean checkout has no node_modules/.vite: the first answer waits for the dependencies to bundle.
  const deadline = Date.now() + STARTUP_LIMIT_MS
  while (Date.now() < deadline) {
    try {
      const signal = AbortSignal.timeout(deadline - Date.now())
      if ((await fetch(url, { signal })).ok) return { vite, url }
    } catch {
      // Not listening yet, or the deadline passed.
    }
    await new Promise((done) => setTimeout(done, 150))
  }
  vite.kill()
  throw new Error(`vite did not start:\n${output}`)
}
