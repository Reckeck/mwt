import type { TourStep } from './tours.ts'

/** The tours of the «Вместе» events: the list, and the screen of one event with its three sections. */
export const TOGETHER_TOURS: Record<string, TourStep[]> = {
  'money-together': [
    { target: 'section-tabs', key: 'planning.tabs', side: 'bottom' },
    { target: 'shared-list', key: 'sharedEvents.list', side: 'top' },
    { target: 'shared-new', key: 'sharedEvents.new', side: 'bottom' },
    { target: 'shared-archive', key: 'sharedEvents.archive', side: 'bottom' },
  ],
  // The event screen: the totals, the three sections, then what each section has.
  'money-together-event': [
    { target: 'shared-status', key: 'sharedEvents.status', side: 'bottom' },
    { target: 'shared-event-menu', key: 'sharedEvents.menu', side: 'bottom' },
    { target: 'shared-totals', key: 'sharedEvents.totals', side: 'bottom' },
    { target: 'shared-shares', key: 'sharedEvents.shares', side: 'bottom' },
    { target: 'shared-tabs', key: 'sharedEvents.tabs', side: 'bottom' },
    { target: 'shared-add-item', key: 'sharedEvents.addItem', side: 'bottom' },
    { target: 'shared-add-bought', key: 'sharedEvents.addBought', side: 'bottom' },
    { target: 'shared-items-table', key: 'sharedEvents.items', side: 'top' },
    { target: 'shared-buy', key: 'sharedEvents.buy', side: 'left' },
    { target: 'shared-add-person', key: 'sharedEvents.addPerson', side: 'bottom' },
    { target: 'shared-overrides-invalid', key: 'sharedEvents.overridesInvalid', side: 'bottom' },
    { target: 'shared-people-table', key: 'sharedEvents.people', side: 'top' },
    { target: 'shared-settlement-summary', key: 'sharedEvents.summary', side: 'bottom' },
    { target: 'shared-my-position', key: 'sharedEvents.myPosition', side: 'bottom' },
    { target: 'shared-settlement', key: 'sharedEvents.settlement', side: 'top' },
    { target: 'shared-pay-share', key: 'sharedEvents.payShare', side: 'bottom' },
    { target: 'shared-share', key: 'sharedEvents.share', side: 'bottom' },
  ],
}
