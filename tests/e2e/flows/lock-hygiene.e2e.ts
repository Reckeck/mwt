import { expect, test } from '../harness/app'

/** Lock hygiene: what the app learned from the vault is gone once it is locked; the unlock screen is shown in place. */
const VAULT = `const vault = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('vault');`
const URL_OF = `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name`
const CACHED = `${URL_OF}
  const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
  const queries = queryClient.getQueryCache().getAll();`

test('a lock drops the query cache and shows the unlock screen, the unlock refetches', async ({ app }) => {
  await app.unlock()
  await app.at('/stores')
  await app.waitFor(`return document.querySelector('[data-purpose-group]') !== null`, 'the accounts')
  expect(
    await app.page.eval<number>(`${CACHED} return queries.filter((query) => query.state.data !== undefined).length`),
  ).toBeGreaterThan(0)

  const lockedAt = await app.page.eval<number>(`${VAULT} const at = Date.now(); await vault.lock(); return at`)
  await app.waitFor(`return document.querySelector('[data-unlock-title]') !== null`, 'the unlock screen')
  // Not a row of the old session is left (the unlock screen's own queries carry no vault data).
  expect(
    await app.page.eval<number>(
      `${CACHED} return queries.filter((query) => query.state.data !== undefined && !['biometric-state', 'recent-vaults'].includes(String(query.queryKey[0]))).length`,
    ),
  ).toBe(0)

  for (const command of ['datasource_preview', 'datasource_test']) {
    const code = await app.page.eval<string | null>(
      `${URL_OF}
       const { call } = await import(urlOf('shared/api/ipc.ts'))
       try { await call('${command}', { url: 'https://example.org/x', draft: {} }); return null } catch (err) { return err.code ?? String(err) }`,
    )
    expect(code).toBe('REQUIRES_UNLOCK')
  }

  await app.page.eval(`${VAULT} await vault.unlock('/tmp/mock.wallet', 'secret'); return true`)
  await app.at('/stores')
  await app.waitFor(`return document.querySelector('[data-purpose-group]') !== null`, 'the accounts again')
  expect(
    await app.page.eval<number>(
      `${CACHED} return queries.filter((query) => query.state.data !== undefined && query.state.dataUpdatedAt < ${lockedAt}).length`,
    ),
  ).toBe(0)
})
