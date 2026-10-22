// Vite on the in-memory mock, on a free port (as check-layout starts it).
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'

export const freePort = () =>
  new Promise((done) => {
    const server = createServer()
    server.listen(0, () => {
      const { port } = server.address()
      server.close(() => done(port))
    })
  })

export async function startVite(root) {
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
