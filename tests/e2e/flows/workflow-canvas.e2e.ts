import { expect, test } from '../harness/app'

// The graph editor keeps its own port and node colours (`--port-*`, `--node-*`): the canvas draws
// them in both themes, and its shots are written to `.cache/e2e/` for a look.
for (const theme of ['light', 'dark'] as const) {
  test(`the scheme canvas draws nodes and wires in the ${theme} theme`, async ({ app }) => {
    await app.unlock()
    await app.at('/automations/workflows/new?template=salaryShare', { theme })
    await app.waitFor(
      `return document.querySelectorAll('.vue-flow__node').length > 1 && document.querySelectorAll('.vue-flow__edge').length > 0`,
      'the template nodes and wires to draw',
    )
    const colours = await app.page.eval<string[]>(`
      const root = getComputedStyle(document.documentElement)
      return ['--node-trigger', '--node-action', '--port-flow', '--port-money'].map((name) => root.getPropertyValue(name).trim())`)
    expect(colours.every((colour) => colour !== '')).toBe(true)
    await app.screenshot(`workflow-canvas-${theme}`)
  })
}
