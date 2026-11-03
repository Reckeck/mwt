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

/** A sum that is not inside a button or a link: a click on it opens the money card. */
// The sidebar's pinned wallets carry sums too, laid over their links: not a plain sum.
const PLAIN_SUM = '[data-money-convertible]:not(a *, button *, label *, [role="button"] *, [role="tab"] *, [data-slot="sidebar"] *)'
const ADD = '[data-tour="quick-add"]'
/** A «⋯» button of an action menu, and an item of the menu it opens (a menu on a desktop, a sheet on a phone). */
const ACTION_MORE = 'button[aria-label="Ещё"]'
const MENU_ITEM = '[role="menuitem"], [data-action-sheet] button'
/** Marks the «⋯» button of the row that has `text` (a click needs a selector; the rows share one). */
const MARK_ROW_MORE = (rows, text) => `
  const row = [...document.querySelectorAll(${JSON.stringify(rows)})].find((node) => node.textContent.includes(${JSON.stringify(text)}))
  row.querySelector('button[aria-label="Ещё"]').setAttribute('data-probe', '')
  return true`
const UNMARK = `document.querySelector('[data-probe]')?.removeAttribute('data-probe'); return true`
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
  // 3.1 Shell
  { id: 'quick-add', route: '/', phone: true, centred: true, open: [{ click: ADD }], phoneOpen: [{ click: '[aria-label="Новая операция"]' }], layer: overlay('quick-add') },
  { id: 'settings', route: '/', phone: true, open: [{ query: { settings: 'app' } }], layer: overlay('settings'), keys: ['settings'] },
  { id: 'palette', route: '/', phone: true, open: [{ key: 'Ctrl+K' }], layer: '[data-command-palette][data-open="true"]' },
  { id: 'converter', route: '/', phone: true, open: [{ eval: flag('/src/shared/lib/converter.ts', 'mod.openConverter()') }], layer: overlay('converter') },
  { id: 'whats-new', route: '/', phone: true, open: [{ query: { whatsNew: '1' } }], layer: overlay('whats-new'), keys: ['whatsNew'] },
  {
    id: 'enrichment-consent', route: '/', phone: true,
    open: [{ eval: PINIA("settings", "enrichmentConsentOpen = true") }],
    layer: overlay('enrichment-consent'),
  },
  { id: 'notifications', route: '/', phone: true, open: [{ click: '[data-tour="notification-bell"]' }], layer: overlay('notifications') },
  { id: 'bottom-more', route: '/', phone: 'only', open: [{ click: '[data-bottom-more]' }], layer: overlay('bottom-more') },
  { id: 'entity-tx', route: '/transactions', phone: true, open: [{ click: '[data-tx-open]' }], layer: '[data-tx-card]', keys: ['open'] },
  { id: 'entity-goal', route: '/money/plan', phone: true, open: [{ query: { open: 'goal:{goal}' } }], layer: '[data-goal-card]', keys: ['open'] },
  { id: 'entity-automation', route: '/automations', phone: true, open: [{ query: { open: 'automation:{automation}' } }], layer: '[data-automation-card]', keys: ['open'] },
  { id: 'entity-card', route: '/', phone: true, open: [{ query: { open: 'category:taxi' } }], layer: overlay('entity-card'), keys: ['open'] },
  { id: 'money-card', route: '/money/month', phone: true, open: [{ click: PLAIN_SUM }], layer: overlay('money-card') },
  { id: 'tx-row-menu', route: '/transactions', open: [{ context: '[data-tx-row]' }], layer: '[data-context-menu]' },
  { id: 'glance-wallet', route: '/transactions', open: [{ click: '[data-glance-trigger="wallet"]' }], layer: overlay('glance-wallet') },
  { id: 'glance-automation', route: '/transactions', phone: true, open: [{ click: '[data-glance-trigger="auto"]' }], layer: overlay('glance-automation') },
  {
    id: 'sandbox-reset', route: '/', phone: true,
    open: [{ eval: PINIA('vault', 'status = { ...store.status, sandbox: true }') }, { click: '[data-sandbox-reset]' }],
    layer: overlay('sandbox-reset'), cleanup: PINIA('vault', 'status = { ...store.status, sandbox: false }'),
  },
  { id: 'tour', route: '/?tour=dashboard', phone: true, fresh: true, open: [{ wait: 1800 }], layer: '.driver-popover', focus: false },

  { id: 'sync-pairing', route: '/', phone: true, open: [{ eval: SYNC_SESSION('pairing'), wait: 600 }], layer: overlay('sync-pairing'), cleanup: SYNC_RESET },
  { id: 'sync-plan', route: '/', phone: 'only', open: [{ eval: SYNC_SESSION('awaiting_confirm'), wait: 600 }], layer: overlay('sync-plan'), cleanup: SYNC_RESET },
  { id: 'entity-link-menu', route: '/transactions', open: [{ context: 'a[data-entity]' }], layer: '[data-context-menu]' },

  // 3.2 Home
  { id: 'glance-capital', route: '/', phone: true, open: [{ click: '[data-glance-trigger="capital"]' }], layer: overlay('glance-capital') },
  { id: 'glance-spent', route: '/', phone: true, open: [{ click: '[data-glance-trigger="spent"]' }], layer: overlay('glance-spent') },
  { id: 'dashboard-catalog', route: '/', phone: true, open: [{ click: '[data-tour="dashboard-add"]' }], layer: overlay('dashboard-catalog') },

  // 3.3 Transactions
  // On a phone the wallets are a list inside the filters panel (money-wallets-filter), not a field of their own.
  { id: 'store-multiselect', route: '/money', open: [{ click: 'button[aria-label="Кошельки"]' }], layer: overlay('store-multiselect') },
  { id: 'tx-save-filter', route: '/transactions?kind=payment', open: [{ click: '[data-tour="tx-save-filter"]' }], layer: overlay('tx-save-filter') },
  { id: 'contact-edit', route: '/contacts', phone: true, open: [{ click: '[data-tour="contact-quick-create"]' }], layer: overlay('contact-edit') },
  { id: 'bonus-program', route: '/transactions/bonuses', phone: true, open: [{ click: '[data-tour="bonuses-add"]' }], layer: overlay('bonus-program') },

  // 3.4 Wallets
  { id: 'store-create', route: '/stores', phone: true, open: [{ click: '[data-tour="stores-create"]' }], layer: overlay('store-create') },
  { id: 'tx-create', route: '/transactions', open: [{ context: '[data-tx-row]' }, { click: '[role="menuitem"]', text: 'Повторить' }], layer: overlay('tx-create') },

  { id: 'wallet-gear', route: '/stores/{store:bank}', phone: true, open: [{ click: '[data-tour="wallet-gear"]' }], layer: '[role="menu"]' },
  {
    id: 'wallet-adjust', route: '/stores/{store:bank}', phone: true,
    open: [{ click: '[data-tour="wallet-gear"]' }, { click: '[data-gear="adjust"]' }],
    layer: overlay('wallet-adjust'),
  },
  { id: 'store-menu', route: '/stores', open: [{ click: '[aria-label^="Действия с"]' }], layer: '[role="menu"]' },
  { id: 'tx-edit', route: '/transactions', open: [{ context: '[data-tx-row]' }, { click: '[role="menuitem"]', text: 'Изменить' }], layer: overlay('tx-edit') },


  // 3.5 Money
  { id: 'category-sheet', route: '/money', phone: true, open: [{ query: { category: 'groceries' } }], layer: overlay('category-sheet'), keys: ['category'] },
  { id: 'budget', route: '/money/plan', phone: true, open: [{ query: { new: 'limit', category: 'groceries' } }], layer: overlay('budget'), keys: ['new', 'category'] },
  { id: 'goal', route: '/money/plan', phone: true, open: [{ click: '[data-tour="plan-goals"] button', text: '^\\s*Цель\\s*$' }], layer: overlay('goal') },
  { id: 'money-wallets-filter', route: '/money', phone: true, open: [{ click: 'button[aria-label="Ещё фильтры"]' }], layer: overlay('money-wallets-filter') },
  { id: 'calc-compare', route: '/money/calculator', phone: true, open: [{ click: '[data-calc-compare]' }], layer: overlay('calc-compare') },

  // 3.5 Money · Together (shared events): the list, the event screen with its three tabs, settling up
  { id: 'shared-event-edit', route: '/money/together', phone: true, centred: true, open: [{ click: '[data-tour="shared-new"]' }], layer: overlay('shared-event-edit') },
  { id: 'shared-event-edit-change', route: '/money/together/{event}', phone: true, open: [{ click: '[data-tour="shared-event-menu"]', wait: 700 }, { click: MENU_ITEM, text: 'Изменить событие' }], layer: overlay('shared-event-edit') },
  { id: 'shared-guest-details', route: '/money/together/{event}?tab=people', phone: true, open: [{ eval: MARK_ROW_MORE('[data-tour="shared-people-table"] tr', 'Оля') }, { click: '[data-probe]', wait: 700 }, { click: MENU_ITEM, text: 'Контакты гостя' }], layer: overlay('shared-guest-details'), cleanup: UNMARK },
  { id: 'shared-item-edit', route: '/money/together/{event}', phone: true, open: [{ click: '[data-tour="shared-add-item"]' }], layer: overlay('shared-item-edit') },
  { id: 'shared-purchase', route: '/money/together/{event}', phone: true, open: [{ click: '[data-tour="shared-buy"]' }], layer: overlay('shared-purchase') },
  { id: 'shared-purchase-unplanned', route: '/money/together/{event}', phone: true, open: [{ click: '[data-tour="shared-add-bought"]' }], layer: overlay('shared-purchase') },
  { id: 'shared-split', route: '/money/together/{event}', phone: true, open: [{ click: `[data-tour="shared-items-table"] ${ACTION_MORE}`, wait: 700 }, { click: MENU_ITEM, text: 'Как делить' }], layer: overlay('shared-split') },
  { id: 'shared-attach', route: '/money/together/{event}', phone: true, open: [{ click: 'button', text: 'Привязать платёж' }], layer: overlay('shared-attach') },
  { id: 'shared-override', route: '/money/together/{event}?tab=people', phone: true, open: [{ click: `[data-tour="shared-people-table"] ${ACTION_MORE}`, wait: 700 }, { click: MENU_ITEM, text: 'Ручной итог' }], layer: overlay('shared-override') },
  { id: 'shared-settle', route: '/money/together/{event}?tab=settle', phone: true, open: [{ click: '[data-tour="shared-settlement"] button', text: '^\\s*Получил\\s*$' }], layer: overlay('shared-settle') },
  { id: 'shared-offset', route: '/money/together/{event}?tab=settle', phone: true, open: [{ click: `[data-tour="shared-settlement"] ${ACTION_MORE}`, wait: 700 }, { click: MENU_ITEM, text: 'Зачесть без операции' }], layer: overlay('shared-settle') },
  { id: 'shared-remainder', route: '/money/together/{event}?tab=settle', phone: true, open: [{ click: `[data-tour="shared-settlement"] ${ACTION_MORE}`, wait: 700 }, { click: MENU_ITEM, text: 'Списать остаток' }], layer: overlay('shared-remainder') },
  { id: 'shared-due', route: '/money/together/{event}?tab=settle', phone: true, open: [{ click: `[data-tour="shared-settlement"] ${ACTION_MORE}`, wait: 700 }, { click: MENU_ITEM, text: 'Срок возврата' }], layer: overlay('shared-due') },
  { id: 'shared-pay-share', route: '/money/together/{event:others}?tab=settle', phone: true, open: [{ click: '[data-tour="shared-pay-share"]' }], layer: overlay('shared-pay-share') },
  { id: 'shared-share', route: '/money/together/{event}?tab=settle', phone: true, open: [{ click: '[data-tour="shared-share"]' }], layer: overlay('shared-share') },

  // 3.6 Automations
  { id: 'automation-wizard', route: '/automations', phone: true, open: [{ query: { new: 'blank' } }], layer: overlay('automation-wizard'), keys: ['new'] },
  { id: 'automation-salary', route: '/automations', phone: true, open: [{ query: { new: 'salary' } }], layer: overlay('automation-salary'), keys: ['new'] },
  { id: 'automation-save-share', route: '/automations', phone: true, open: [{ query: { new: 'save_share' } }], layer: overlay('automation-save-share'), keys: ['new'] },
  { id: 'reminder', route: '/automations/reminders', phone: true, open: [{ query: { new: 'reminder' } }], layer: overlay('reminder'), keys: ['new'] },
  { id: 'price-alert', route: '/automations/alerts', phone: true, open: [{ query: { new: 'alert' } }], layer: overlay('price-alert'), keys: ['new'] },
  { id: 'automation-edit', route: '/automations?edit={automation}', phone: true, fresh: true, open: [{ wait: 900 }], layer: overlay('automation-edit'), keys: ['edit'] },
  { id: 'confirm', route: '/automations/reminders', phone: true, open: [{ click: '[data-slot="confirm-button"]' }], layer: overlay('confirm') },
  { id: 'workflow-import', route: '/automations', open: [{ click: '[data-tour="automations-import"]' }], layer: overlay('workflow-import') },
  { id: 'workflow-vars', route: '/automations', phone: true, open: [{ click: '[data-tour="workflow-vars-open"]' }], layer: overlay('workflow-vars') },

  // 3.7 Settings (windows inside ?settings=)
  { id: 'password-change', route: '/?settings=vault&section=file', phone: true, open: [{ click: 'button', text: 'Сменить пароль' }], layer: overlay('password-change') },
  { id: 'backup-restore', route: '/?settings=vault&section=file', phone: true, open: [{ click: 'button', text: 'Восстановить' }], layer: overlay('backup-restore') },
  { id: 'category-add', route: '/?settings=accounting&section=categories', phone: true, open: [{ click: '[data-category-add]' }], layer: overlay('category-add') },

  // 3.8 Lock screen
  { id: 'vault-create', route: '/unlock', locked: true, phone: true, open: [{ click: 'button', text: 'Создать новое' }], layer: overlay('vault-create') },
  { id: 'vault-receive', route: '/unlock', locked: true, phone: true, open: [{ click: 'button', text: 'Получить с другого' }], layer: overlay('vault-receive') },
  { id: 'app-settings-unlock', route: '/unlock', locked: true, phone: true, open: [{ click: 'button[aria-label="Настройки"]' }], layer: overlay('app-settings-unlock') },
  { id: 'recent-vaults', route: '/unlock', locked: true, phone: true, open: [{ click: '[data-tour="unlock-recents"] button' }], layer: overlay('recent-vaults') },
]
