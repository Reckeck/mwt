// Shared by the browser gates (e2e, check-layout, check-links): a free port and a Vite dev server on it.
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'

const root = new URL('../../..', import.meta.url).pathname

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
    stdio: 'ignore',
  })
  const url = `http://localhost:${port}/`
  for (let tries = 0; tries < 200; tries++) {
    try {
      if ((await fetch(url)).ok) return { vite, url }
    } catch {
      await new Promise((done) => setTimeout(done, 150))
    }
  }
  vite.kill()
  throw new Error('vite did not start')
}
