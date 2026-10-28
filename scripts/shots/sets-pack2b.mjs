/** Pack 2 · Money and «Ещё»: bonuses and investments on a phone, the calculator, the month labels, the menu descriptions. */
export const pack2b = [
  { name: 'p2b-bonuses', route: '/transactions/bonuses', steps: [{ wait: 900 }] },
  { name: 'p2b-invest', route: '/money/investments', steps: [{ wait: 900 }] },
  { name: 'p2b-calc', route: '/money/calculator', steps: [{ wait: 1500 }, { eval: `document.querySelector('[data-calc-result]')?.scrollIntoView({ block: 'center' }); return true` }, { wait: 300 }] },
  {
    name: 'p2b-calc-withdrawal',
    route: '/money/calculator',
    steps: [
      { wait: 900 },
      { click: '[data-calc-advanced] button' },
      { click: '#calc-withdrawal-on' },
      { fill: ['#calc-withdrawal', '30000'] },
      { wait: 1200 },
      { eval: `document.querySelector('[data-calc-result]')?.scrollIntoView({ block: 'center' }); return true` },
      { wait: 300 },
    ],
  },
  { name: 'p2b-spending', route: '/money', steps: [{ wait: 1000 }] },
  { name: 'p2b-spending-12', route: '/money?from=2025-11&to=2026-10', steps: [{ wait: 1000 }] },
  { name: 'p2b-money-more', route: '/money', steps: [{ wait: 700 }, { click: '[data-tour="route-tabs-more"]' }, { wait: 500 }] },
  { name: 'p2b-more-sheet', route: '/', desktop: false, steps: [{ wait: 700 }, { phone: [{ click: '[data-bottom-more]' }] }, { wait: 500 }] },
]
