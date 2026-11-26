// What check-layout opens (docs/ui-overlays.md §3, §6). One entry per layer:
//   id      — the layer's `data-overlay` (and its name in the doc)
//   route   — where it opens; {goal}, {automation}, {store:<kind>} are filled from the mock
//   open    — steps: { click | context | hover: selector, text? } | { key: 'Ctrl+K' } | { query } | { eval } (+ wait ms)
//   phoneOpen — other steps at 390 (the header button is hidden there)
//   layer   — selector of the open layer
//   fresh   — arrive from another page first: the layer opens when its page mounts (?edit=, a tour)
//   locked  — on the lock screen (/unlock): the vault is locked for these and opened again after them
//   phone   — true: also at 390; 'only': only at 390
//   cleanup — code that undoes what the steps set (a store flag)
//   keys    — query keys that must be gone once it is closed
//   centred — a centred dialog: its centre must not move while it opens
//   focus / touch / modalShift — false/false/true to switch a check off for a layer that is not modal, small, or locks scrolling
const overlay = (id) => `[data-overlay="${id}"]`

/** Sets a field of a pinia store. */
const PINIA = (name, assign) => `const store = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('${name}'); store.${assign}; return true`
const flag = (module, code) => `const mod = await import('${module}'); ${code}; return true`

/**
 * A second device in the sync status, set in the query cache only (the mock backend keeps no pairing behind it,
 * so the sync views that follow see the vault as it was). `phase`: 'pairing' (the code) or 'awaiting_confirm' (the plan).
 */
const SYNC_SESSION = (phase) => `
  const provides = document.querySelector('#app').__vue_app__._context.provides
  const client = Reflect.ownKeys(provides).map((key) => provides[key]).find((value) => typeof value?.setQueryData === 'function')
  const { call } = await import('/src/shared/api/ipc.ts')
  const status = await call('sync_status')
  const summary = {
    total: 3,
    collections: [{ collection: 'transactions', added: 2, updated: 1, deleted: 0, items: [{ kind: 'added', label: 'A' }, { kind: 'added', label: 'B' }, { kind: 'updated', label: 'C' }] }],
    overwritten: [{ collection: 'stores', label: 'Cash', field: 'name', before: 'Wallet', after: 'Cash' }],
  }
  const session = {
    session_id: 'check-layout', device_id: 'check-layout-phone', name: 'Pixel 8', phase: '${phase}', unreliable: false, clock_offset_ms: 0,
    progress: null, focused: false, pairing_code: '${phase}' === 'pairing' ? '482913' : null,
    pairing_expires_ms: '${phase}' === 'pairing' ? Date.now() + 120000 : null,
    plan: '${phase}' === 'awaiting_confirm' ? { reason: 'paired', summary } : null,
  }
  client.setQueryData(['sync-status'], { ...status, enabled: true, role: 'main', sessions: [session] })
  return true`
const SYNC_RESET = `
  const provides = document.querySelector('#app').__vue_app__._context.provides
  const client = Reflect.ownKeys(provides).map((key) => provides[key]).find((value) => typeof value?.setQueryData === 'function')
  await client.invalidateQueries({ queryKey: ['sync-status'] })
  return true`

export const OVERLAYS = [
  // 3.1 Shell: what is left while the screens are rebuilt (T72). Every screen's own layers come back with its W4 task.
  { id: 'palette', route: '/', phone: true, open: [{ key: 'Ctrl+K' }], layer: '[data-command-palette][data-open="true"]' },
  { id: 'converter', route: '/', phone: true, open: [{ eval: flag('/src/shared/lib/converter.ts', 'mod.openConverter()') }], layer: overlay('converter') },
  {
    id: 'enrichment-consent', route: '/', phone: true,
    open: [{ eval: PINIA("settings", "enrichmentConsentOpen = true") }],
    layer: overlay('enrichment-consent'),
  },
  { id: 'profile-menu', route: '/', phone: true, open: [{ click: '[data-profile-trigger]' }], layer: overlay('profile-menu') },
  { id: 'entity-goal', route: '/money/plan', phone: true, open: [{ query: { open: 'goal:{goal}' } }], layer: '[data-goal-card]', keys: ['open'] },
  { id: 'entity-card', route: '/', phone: true, open: [{ query: { open: 'category:taxi' } }], layer: overlay('entity-card'), keys: ['open'] },
  {
    id: 'sandbox-reset', route: '/', phone: true,
    open: [{ eval: PINIA('vault', 'status = { ...store.status, sandbox: true }') }, { click: '[data-sandbox-reset]' }],
    layer: overlay('sandbox-reset'), cleanup: PINIA('vault', 'status = { ...store.status, sandbox: false }'),
  },
  // 3.2 Unlock screen (T41). vault-receive and app-settings-unlock return with T42.
  { id: 'vault-create', route: '/unlock', locked: true, phone: true, open: [{ click: '[data-unlock-create]' }], layer: overlay('vault-create') },
  { id: 'vault-unlock', route: '/unlock', locked: true, phone: true, open: [{ click: '[data-recents-other]' }], layer: overlay('vault-unlock') },
  // Money: the rebuilt plan and calculator
  { id: 'budget', route: '/money/plan', phone: true, open: [{ click: '[data-budget-new]' }], layer: overlay('budget') },
  { id: 'goal', route: '/money/plan', phone: true, open: [{ click: '[data-plan-goal]' }], layer: overlay('goal') },
  { id: 'calc-compare', route: '/money/calculator', phone: true, open: [{ click: '[data-calc-compare]' }], layer: overlay('calc-compare') },
  { id: 'sync-pairing', route: '/', phone: true, open: [{ eval: SYNC_SESSION('pairing'), wait: 600 }], layer: overlay('sync-pairing'), cleanup: SYNC_RESET },
  { id: 'sync-plan', route: '/', phone: 'only', open: [{ eval: SYNC_SESSION('awaiting_confirm'), wait: 600 }], layer: overlay('sync-plan'), cleanup: SYNC_RESET },
]
