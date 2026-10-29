/** Stage 5 · Automations (Auto-1…3): `Mine`, the card, the journal, the wizard, schemes. */

/** Into step 2 with a transfer: the default type is a repeat, so one click moves on. */
const WIZARD_DO = [{ wait: 400 }, { click: '[data-wizard-type="repeat"]' }, { click: '[data-kind-tab="transfer"]' }]

/** A file from someone else's vault: two steps and their `Sber`, unknown here (Auto-3 · 17). */
const IMPORT_FILE = `const file = new File([JSON.stringify({
  format: 'trade-app-workflow', version: 1, name: 'Платёж в рабочий день', description: '',
  graph: { nodes: [{ id: 't', kind: 'trigger.schedule' }, { id: 'a', kind: 'action.transfer' }, { id: 'b', kind: 'action.notify' }], edges: [] },
  stores: [{ id: 'foreign-sber', name: 'Сбер' }],
})], 'payment.json', { type: 'application/json' })
const input = document.querySelector('[data-import-input]'); const data = new DataTransfer(); data.items.add(file)
input.files = data.files; input.dispatchEvent(new Event('change', { bubbles: true })); return true`

export const stage5 = [
  { name: 'auto-mine', route: '/automations', canvas: { board: 'Auto-1', frame: '1.' } },
  { name: 'auto-mine-empty', route: '/automations', scenarios: ['empty'], canvas: { board: 'Auto-1', frame: '2.' } },
  { name: 'auto-card', route: '/automations', steps: [{ click: '[data-mine-row^="scheme"] a' }, { wait: 500 }], canvas: { board: 'Auto-1', frame: '3.' } },
  { name: 'auto-journal', route: '/automations/journal', canvas: { board: 'Auto-1', frame: '4.' } },
  {
    name: 'auto-confirm',
    route: '/automations/journal',
    steps: [{ click: '[data-journal-confirm]' }, { wait: 500 }],
    canvas: { board: 'Auto-1', frame: '5.' },
  },
  { name: 'auto-wizard-1', route: '/automations?new=blank', steps: [{ wait: 500 }], canvas: { board: 'Auto-1', frame: '6.' } },
  {
    name: 'auto-wizard-2',
    route: '/automations?new=blank',
    steps: [...WIZARD_DO, { wait: 400 }],
    canvas: { board: 'Auto-1', frame: '7.' },
  },
  {
    name: 'auto-wizard-3',
    route: '/automations?new=blank',
    steps: [{ wait: 400 }, { click: '[data-wizard-type="repeat"]' }, { fill: ['#tx-amount', '600'] }, { click: '[data-wizard-next]' }, { click: '[data-wizard-shift="back"]' }, { click: '#automation-wizard-asks' }, { wait: 800 }],
    canvas: { board: 'Auto-2', frame: '8.' },
  },
  {
    name: 'auto-save-share',
    route: '/automations?new=save_share',
    steps: [{ wait: 800 }],
    canvas: { board: 'Auto-2', frame: '12.' },
  },
  {
    name: 'auto-salary',
    route: '/automations?new=salary',
    steps: [{ wait: 500 }, { fill: ['#salary-0-amount', '1575'] }, { wait: 300 }],
    canvas: { board: 'Auto-2', frame: '9.' },
  },
  {
    name: 'auto-reminder',
    route: '/automations?new=reminder',
    steps: [{ wait: 500 }, { fill: ['#reminder-text', 'Заплатить ЖКХ'] }, { wait: 300 }],
    canvas: { board: 'Auto-2', frame: '10.' },
  },
  {
    name: 'auto-scheme',
    route: '/automations',
    steps: [{ click: '[data-mine-row^="scheme"] a' }, { wait: 500 }, { click: '[data-automation-edit]' }, { wait: 1500 }],
    canvas: { board: 'Auto-2', frame: '13.' },
  },
  {
    name: 'auto-scheme-node',
    route: '/automations',
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { desktop: [{ click: '.vue-flow__node[data-id="save"]' }, { hover: '[data-tour="workflow-words"]' }] },
      { phone: [{ click: '[data-step="save"]' }] },
      { wait: 600 },
    ],
    canvas: { board: 'Auto-2', frame: '13.' },
  },
  {
    name: 'auto-scheme-canvas',
    route: '/automations',
    desktop: false,
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { click: '[data-steps-canvas]' },
      { wait: 800 },
    ],
  },
  {
    name: 'auto-scheme-add',
    route: '/automations',
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { desktop: [{ click: '.vue-flow__edge[data-id="income-share-a"]' }, { press: 'shift+a' }] },
      { phone: [{ click: '[data-steps-add]' }] },
      { wait: 500 },
      { type: ['[data-tour="workflow-quick-add"] input', '10%'] },
      { wait: 400 },
    ],
    canvas: { board: 'Auto-2', frame: '14.' },
  },
  {
    name: 'auto-scheme-run',
    route: '/automations',
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { click: '[data-tour="workflow-emulate"]' },
      { wait: 1200 },
    ],
    canvas: { board: 'Auto-3', frame: '15.' },
  },
  {
    name: 'auto-scheme-error',
    route: '/automations',
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { desktop: [{ click: '.vue-flow__node[data-id="save"]' }] },
      { phone: [{ click: '[data-step="save"]' }] },
      { wait: 500 },
      { click: '[data-param="to"] button' },
      { wait: 300 },
      { click: '[role=option]' },
      { wait: 600 },
      { phone: [{ press: 'Escape' }, { wait: 500 }] },
    ],
    canvas: { board: 'Auto-3', frame: '16.' },
  },
  {
    name: 'auto-import',
    route: '/automations/workflows',
    steps: [
      { click: '[data-workflow-import]' },
      { wait: 500 },
      { eval: IMPORT_FILE },
      { wait: 800 },
    ],
    canvas: { board: 'Auto-3', frame: '17.' },
  },
  {
    name: 'auto-scheme-yes',
    route: '/automations',
    steps: [
      { click: '[data-mine-row^="scheme"] a' },
      { wait: 500 },
      { click: '[data-automation-edit]' },
      { wait: 1500 },
      { click: '[data-tour="workflow-emulate"]' },
      { wait: 1200 },
      { click: '[data-run-answer="yes"]' },
      { wait: 1200 },
    ],
  },
  {
    name: 'auto-alert',
    route: '/automations?new=alert',
    steps: [{ wait: 600 }, { type: ['#price-alert-form input', 'btc'] }, { wait: 900 }, { click: '[role="option"]' }, { wait: 500 }, { fill: ['#alert-price', '90000'] }, { wait: 800 }],
    canvas: { board: 'Auto-2', frame: '11.' },
  },
]
