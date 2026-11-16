/**
 * Commands: the endpoints of each network and their connection test.
 *
 * Owner: spec E17. The mock keeps what the application keeps on the device: the user's own
 * addresses per network and whether a Blockscout key is stored (never the key itself, nothing
 * returns it). It applies the same address rules as Rust, and like the application it refuses the
 * connection test without the tracking consent. The presets are the table of `cryptotrack/chains.rs`.
 */

import { type MockCommands, resolve } from './core'
import { cryptoSyncCommands } from './commands-crypto-sync'

interface Preset {
  chain: string
  label: string
  chain_id: number
  rpc: string[]
  blockscout: string[]
  explorer: string
}

const PRESETS: Preset[] = [
  {
    chain: 'ethereum',
    label: 'Ethereum',
    chain_id: 1,
    rpc: ['https://ethereum-rpc.publicnode.com', 'https://eth.drpc.org'],
    blockscout: ['https://eth.blockscout.com'],
    explorer: 'https://etherscan.io',
  },
  {
    chain: 'arbitrum',
    label: 'Arbitrum',
    chain_id: 42161,
    rpc: ['https://arb1.arbitrum.io/rpc', 'https://arbitrum-one-rpc.publicnode.com'],
    blockscout: ['https://arbitrum.blockscout.com'],
    explorer: 'https://arbiscan.io',
  },
  {
    chain: 'optimism',
    label: 'Optimism',
    chain_id: 10,
    rpc: ['https://mainnet.optimism.io', 'https://optimism-rpc.publicnode.com'],
    blockscout: ['https://explorer.optimism.io'],
    explorer: 'https://optimistic.etherscan.io',
  },
  {
    chain: 'base',
    label: 'Base',
    chain_id: 8453,
    rpc: ['https://mainnet.base.org', 'https://base-rpc.publicnode.com'],
    blockscout: ['https://base.blockscout.com'],
    explorer: 'https://basescan.org',
  },
  {
    chain: 'polygon',
    label: 'Polygon',
    chain_id: 137,
    rpc: ['https://polygon.drpc.org', 'https://polygon-bor-rpc.publicnode.com'],
    blockscout: ['https://polygon.blockscout.com'],
    explorer: 'https://polygonscan.com',
  },
  {
    chain: 'bsc',
    label: 'BNB Smart Chain',
    chain_id: 56,
    rpc: ['https://bsc-dataseed.bnbchain.org', 'https://bsc-rpc.publicnode.com'],
    blockscout: [],
    explorer: 'https://bscscan.com',
  },
]

interface Own {
  rpc: string
  blockscout: string
  explorer: string
  key: boolean
}

const own = new Map<string, Own>()
let consentAt: string | undefined

const invalid = (message: string) => Promise.reject({ code: 'INVALID_INPUT', message })

const isOwnNode = (host: string) => host === 'localhost' || host === '127.0.0.1' || host === '[::1]'

/** The rules of `service/cryptotrack/network.rs`: https, or http to localhost; no credentials, no fragment. */
function checkEndpoint(raw: string, allowQuery: boolean): string | null {
  const text = raw.trim()
  if (!text) return null
  if (/[\s\\]/.test(text)) return 'network address: it contains whitespace or control characters'
  let url: URL
  try {
    url = new URL(text)
  } catch {
    return 'network address: it is not a valid address'
  }
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && isOwnNode(url.hostname))) {
    return 'network address: only https:// is accepted (http:// only for localhost)'
  }
  if (url.username || url.password) return 'network address: it must not carry a user name or password'
  if (url.hash) return 'network address: a fragment (#) is not allowed'
  if (!allowQuery && url.search) return 'network address: a query (?) is not allowed'
  return null
}

function checkExplorer(raw: string): string | null {
  const text = raw.trim()
  if (!text) return null
  if (!/^https:\/\/[a-z0-9-]+(\.[a-z0-9-]+)+(:\d+)?(\/[^?#\s]*)?$/i.test(text))
    return 'explorer URL: only https:// links are allowed'
  return null
}

const endpoint = (custom: string, presets: string[]) => {
  const value = custom.trim().replace(/\/$/, '') || presets[0] || ''
  let host = ''
  try {
    host = value ? new URL(value).host : ''
  } catch {
    host = ''
  }
  return { value, custom: Boolean(custom.trim()), presets, host }
}

const view = (preset: Preset) => {
  const mine = own.get(preset.chain)
  return {
    chain: preset.chain,
    label: preset.label,
    chain_id: preset.chain_id,
    rpc: endpoint(mine?.rpc ?? '', preset.rpc),
    blockscout: endpoint(mine?.blockscout ?? '', preset.blockscout),
    explorer: endpoint(mine?.explorer ?? '', [preset.explorer]),
    blockscout_key_set: mine?.key ?? false,
  }
}

export const cryptoNetworkCommands: MockCommands = {
  crypto_network_settings: async () => {
    const status = (await cryptoSyncCommands.crypto_sync_status({
      cmd: 'crypto_sync_status',
      args: undefined,
      body: undefined,
    })) as {
      consent: boolean | null
    }
    // The application stores the date on this device when the consent is given.
    const given = status.consent === true
    if (given && !consentAt) consentAt = new Date().toISOString()
    if (!given) consentAt = undefined
    return resolve({
      editable: true,
      networks: PRESETS.map(view),
      consent_at: consentAt,
      consent_device: given ? 'Laptop' : undefined,
    })
  },
  crypto_set_network_settings: ({ body }) => {
    const input = body as {
      chain: string
      rpc: string
      blockscout: string
      explorer: string
      blockscout_key?: string | null
    }
    const problem =
      checkEndpoint(input.rpc, true) ?? checkEndpoint(input.blockscout, false) ?? checkExplorer(input.explorer)
    if (problem) return invalid(problem)
    const key = input.blockscout_key
    if (typeof key === 'string' && key.trim() && !/^[\x21-\x7e]{1,256}$/.test(key.trim())) {
      return invalid('the Blockscout key must be 1-256 visible ASCII characters without spaces')
    }
    const before = own.get(input.chain)
    const keyStored = key == null ? (before?.key ?? false) : key.trim() !== ''
    const next = {
      rpc: input.rpc.trim(),
      blockscout: input.blockscout.trim(),
      explorer: input.explorer.trim(),
      key: keyStored,
    }
    if (next.rpc || next.blockscout || next.explorer || next.key) own.set(input.chain, next)
    else own.delete(input.chain)
    return resolve(null)
  },
  crypto_test_network: async ({ body }) => {
    const input = body as { chain: string; kind: 'rpc' | 'blockscout'; url?: string | null }
    const status = (await cryptoSyncCommands.crypto_sync_status({
      cmd: 'crypto_sync_status',
      args: undefined,
      body: undefined,
    })) as { consent: boolean | null }
    if (status.consent !== true) return invalid('reading the addresses is turned off')
    const preset = PRESETS.find((entry) => entry.chain === input.chain)
    if (!preset) return invalid('unsupported')
    const typed = input.url?.trim() ?? ''
    const problem = typed ? (input.kind === 'rpc' ? checkEndpoint(typed, true) : checkEndpoint(typed, false)) : null
    if (problem) return invalid(problem)
    const mine = own.get(input.chain)
    const url = typed || (input.kind === 'rpc' ? mine?.rpc : mine?.blockscout) || preset[input.kind][0] || ''
    if (!url) return invalid('this network has no Blockscout instance to test')
    const unreachable = url.includes('unreachable')
    return resolve({
      chain: input.chain,
      kind: input.kind,
      outcome: unreachable ? 'unreachable' : 'ok',
      latency_ms: 80,
      block: input.kind === 'rpc' && !unreachable ? 24_118_902 : undefined,
    })
  },
}
