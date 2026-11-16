/** `What's new` of the crypto network settings (spec E17): its own file, so the stage's specs do not share lines. */
import type { WhatsNewEntry } from './whats-new.types'

export const WHATS_NEW_CRYPTO_NETWORK: WhatsNewEntry[] = [
  {
    id: 'cryptoPositions',
    date: '2026-11-16',
    points: [
      { key: 'screen', route: 'crypto-positions' },
      { key: 'unknown', route: 'crypto-positions' },
    ],
  },
  {
    id: 'cryptoWipe',
    date: '2026-11-14',
    points: [
      { key: 'backup', route: 'settings:crypto:wipe' },
      { key: 'wallets', route: 'settings:crypto:wipe' },
    ],
  },
  {
    id: 'cryptoManualEnd',
    date: '2026-11-14',
    points: [{ key: 'forms' }, { key: 'readable' }],
  },
  {
    id: 'cryptoBybit',
    date: '2026-11-14',
    points: [{ key: 'moved' }, { key: 'queue' }],
  },
  {
    id: 'cryptoEngine',
    date: '2026-11-14',
    points: [
      { key: 'follow', route: 'stores' },
      { key: 'tokens', route: 'wallet' },
    ],
  },
  {
    id: 'cryptoTransition',
    date: '2026-11-14',
    points: [{ key: 'warning' }, { key: 'archive' }],
  },
  {
    id: 'cryptoNetworkSettings',
    date: '2026-11-12',
    points: [
      { key: 'nodes', route: 'settings:crypto:networks' },
      { key: 'key', route: 'settings:crypto:networks' },
      { key: 'consent', route: 'settings:crypto:consent' },
      { key: 'tokens', route: 'settings:crypto:tokens' },
    ],
  },
]
