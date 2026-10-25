/** Stage 6¾ · Money (docs/ui-money.md): a sum shows the other currencies on hover, a card on a tap or a hold. */

const PLAIN = '[data-money-convertible]:not(:is(a, button, [role="button"], [role="menuitem"]) *)'
const IN_BUTTON = ':is(a, button) [data-money-convertible]'

export const money = [
  { name: 'money-hover-text', route: '/', phone: false, steps: [{ hover: PLAIN }, { wait: 700 }] },
  { name: 'money-hover-dated', route: '/transactions', phone: false, steps: [{ hover: '[data-tx-row] [data-money-convertible]' }, { wait: 700 }] },
  { name: 'money-hover-button', route: '/', phone: false, steps: [{ hover: IN_BUTTON }, { wait: 700 }] },
  { name: 'money-tap-card', route: '/', desktop: false, steps: [{ tap: PLAIN }, { wait: 500 }] },
  { name: 'money-hold-button', route: '/stores', desktop: false, steps: [{ hold: IN_BUTTON }, { wait: 500 }] },
  { name: 'money-hold-row', route: '/transactions', desktop: false, steps: [{ hold: '[data-tx-row]' }, { wait: 500 }] },
]
