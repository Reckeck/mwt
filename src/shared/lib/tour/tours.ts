/** One stop of a tour: the element (by `data-tour`) and the i18n keys of its popover. */
export interface TourStep {
  target: string
  key: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}

/**
 * Tours by route name. Steps whose element is not on the screen are skipped,
 * so one tour serves wallets of every kind.
 */
export const TOURS: Record<string, TourStep[]> = {
  dashboard: [
    { target: 'dashboard-grid', key: 'dashboard.grid', side: 'top' },
    { target: 'dashboard-add', key: 'dashboard.add', side: 'left' },
    { target: 'sidebar-add-wallet', key: 'dashboard.wallets', side: 'right' },
    { target: 'open-settings', key: 'dashboard.settings', side: 'bottom' },
    { target: 'tour-help', key: 'dashboard.contextMenu', side: 'bottom' },
  ],
  wallet: [
    { target: 'wallet-adjust', key: 'wallet.adjust', side: 'bottom' },
    { target: 'wallet-settings', key: 'wallet.settings', side: 'bottom' },
    { target: 'exchange-views', key: 'wallet.views', side: 'bottom' },
    { target: 'exchange-terminal', key: 'wallet.terminal', side: 'top' },
  ],
  'wallet-settings': [
    { target: 'wallet-purpose', key: 'walletSettings.purpose', side: 'top' },
    { target: 'wallet-envelopes', key: 'walletSettings.envelopes', side: 'top' },
    { target: 'automation-presets', key: 'walletSettings.presets', side: 'bottom' },
  ],
  'directory-contacts': [
    { target: 'contact-quick-create', key: 'directory.create', side: 'right' },
    { target: 'contact-editor', key: 'directory.editor', side: 'left' },
    { target: 'contact-debts', key: 'directory.debts', side: 'top' },
  ],
}
