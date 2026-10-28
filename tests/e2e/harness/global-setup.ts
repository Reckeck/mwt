// Once per e2e run: Vite on the in-memory mock (a free port) and one headless Chromium.
// Test files attach to them through `inject('e2e')` (see app.ts).
import type { TestProject } from 'vitest/node'

import { findChromium, spawnBrowser } from './cdp.mjs'
import { freePort, startVite } from './server.mjs'

export interface E2eEnv {
  url: string
  debugPort: number
}

declare module 'vitest' {
  export interface ProvidedContext {
    e2e: E2eEnv
  }
}

export default async function setup(project: TestProject) {
  const binary = findChromium()
  if (!binary) throw new Error('e2e: no Chromium (set CHROMIUM=/path/to/chromium)')
  const { vite, url } = await startVite()
  const debugPort = await freePort()
  const browser = await spawnBrowser(binary, debugPort)
  project.provide('e2e', { url, debugPort })
  // Warm Vite up: the first load pre-bundles dependencies and may reload the page.
  await fetch(url)
  return async () => {
    await browser.stop()
    vite.kill()
  }
}
