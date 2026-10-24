/** Stage 5 · Automations (Auto-1…3): «Мои», the card, the journal, the wizard, schemes. */

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
]
