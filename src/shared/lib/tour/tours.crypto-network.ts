import type { TourStep } from './tours.types.ts'

/**
 * Settings -> Crypto steps of the settings tour (spec E17): the networks and their nodes, the optional Blockscout
 * key, the one-time consent, the hidden tokens and the way to wiping. A step whose tab is not open is skipped.
 */
export const CRYPTO_NETWORK_TOUR_STEPS: TourStep[] = [
  { target: 'crypto-network-list', key: 'cryptoNetwork.list', side: 'top' },
  { target: 'crypto-network-form', key: 'cryptoNetwork.form', side: 'top' },
  { target: 'crypto-network-presets', key: 'cryptoNetwork.presets', side: 'top' },
  { target: 'crypto-network-key', key: 'cryptoNetwork.key', side: 'top' },
  { target: 'crypto-network-sources', key: 'cryptoNetwork.sources', side: 'top' },
  { target: 'crypto-consent', key: 'cryptoNetwork.consent', side: 'top' },
  { target: 'crypto-hidden-tokens', key: 'cryptoNetwork.hidden', side: 'top' },
  { target: 'crypto-wipe-backup', key: 'cryptoNetwork.wipeBackup', side: 'top' },
  { target: 'crypto-wipe', key: 'cryptoNetwork.wipe', side: 'top' },
]
