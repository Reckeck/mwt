/** Stage 5 · Automations (Auto-1…3): «Мои», the card, the journal, the wizard, schemes. */

/** Into step 2 with a transfer: the default type is a repeat, so one click moves on. */
const WIZARD_DO = [{ wait: 400 }, { click: '[data-wizard-type="repeat"]' }, { click: '[data-kind-tab="transfer"]' }]

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
]
