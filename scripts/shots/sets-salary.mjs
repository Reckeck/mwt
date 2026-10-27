/** Task 219 · «Зарплата» as a superset form: one payout a month or two, and «за <месяц>» on the income's row. */

const SCROLL_TO_BADGE = `document.querySelector('[data-tx-for-month]')?.scrollIntoView({ block: 'center' }); return true`
const OPEN_BADGED = `const badge = document.querySelector('[data-tx-for-month]'); badge?.scrollIntoView({ block: 'center' }); badge?.closest('.min-w-0.flex-1')?.querySelector('[data-tx-open]')?.click(); return true`

export const salary = [
  {
    name: 'salary-1',
    route: '/automations?new=salary',
    steps: [{ wait: 500 }, { fill: ['#salary-0-amount', '1575'] }, { wait: 300 }],
  },
  {
    name: 'salary-2',
    route: '/automations?new=salary',
    steps: [
      { wait: 500 },
      { click: '[data-salary-count="2"]' },
      { fill: ['#salary-0-amount', '1500'] },
      { fill: ['#salary-1-amount', '500'] },
      { wait: 400 },
    ],
  },
  {
    name: 'salary-2-top',
    route: '/automations?new=salary',
    steps: [{ wait: 500 }, { click: '[data-salary-count="2"]' }, { wait: 400 }],
  },
  {
    name: 'salary-2-end',
    route: '/automations?new=salary',
    steps: [
      { wait: 500 },
      { click: '[data-salary-count="2"]' },
      { eval: `const body = document.querySelector('[data-dialog-body]'); body.scrollTop = body.scrollHeight; return true` },
      { wait: 400 },
    ],
  },
  {
    name: 'salary-2-errors',
    route: '/automations?new=salary',
    steps: [{ wait: 500 }, { click: '[data-salary-count="2"]' }, { click: { text: 'Создать', within: '[role="dialog"]' } }, { wait: 500 }],
  },
  { name: 'salary-row', route: '/transactions', steps: [{ wait: 800 }, { eval: SCROLL_TO_BADGE }, { wait: 300 }] },
  { name: 'salary-card', route: '/transactions', steps: [{ wait: 800 }, { eval: OPEN_BADGED }, { wait: 800 }] },
]
