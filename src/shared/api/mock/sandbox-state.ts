/**
 * The sandbox of the browser mock (`open_sandbox`, `reset_sandbox`, `close_sandbox`, `is_sandbox` in Rust).
 *
 * The mock keeps its data in module-level lists, so the sandbox swaps their contents: opening keeps the
 * real mock vault aside and plays the demo data (the mock as it was seeded, before anyone edited it),
 * «Сбросить» plays that demo again, closing brings the real data back. Like the Rust side, the sandbox
 * is never among the recent vaults, and what talks to the outside world is refused in it.
 */

import { mockCategories, mockContacts, mockRules } from './ledger'
import { mockStores, mockTransactions } from './demo'
import * as planning from './planning/state'
import { mockInfo, mockSettings, mockStatus } from './vault-data'

/** The lists and objects a sandbox edit may change. */
const LISTS: unknown[][] = [
  mockStores,
  mockTransactions,
  mockCategories,
  mockContacts,
  mockRules,
  planning.automations,
  planning.runs,
  planning.budgets,
  planning.goals,
  planning.reminders,
  planning.alerts,
  planning.utilities,
  planning.readings,
]

interface Snapshot {
  lists: unknown[][]
  settings: Settings
}

const copy = <Value>(value: Value): Value => JSON.parse(JSON.stringify(value)) as Value

const capture = (): Snapshot => ({ lists: LISTS.map((list) => copy(list)), settings: copy(mockSettings) })

function restore(snapshot: Snapshot) {
  for (const [index, list] of LISTS.entries()) list.splice(0, list.length, ...copy(snapshot.lists[index]))
  Object.assign(mockSettings, copy(snapshot.settings))
}

/** The mock as it was seeded: what a fresh sandbox shows. */
let pristine: Snapshot | null = null
/** The real mock vault, while the sandbox is open. */
let aside: Snapshot | null = null

/** Called once the mock is seeded, before any command can change it. */
export function keepPristine() {
  pristine ??= capture()
}

/** Where the Rust side keeps it; shown as the vault's path. */
const SANDBOX_PATH = '/tmp/mock-cache/sandbox/sandbox.wallet'

const refusal = (what: string) => ({ code: 'SANDBOX', message: `sandbox: ${what} is off in the sandbox` })

export const isSandbox = () => mockStatus.value.sandbox

function infoOf(): VaultInfo {
  return { ...mockInfo, path: SANDBOX_PATH, encrypted: false }
}

function show(info: VaultInfo | null, sandbox: boolean) {
  mockStatus.value = {
    ...mockStatus.value,
    unlocked: info !== null,
    info,
    encrypted: info?.encrypted ?? mockInfo.encrypted,
    sandbox,
  }
}

/** A real vault open: the sandbox does not replace it (lock it first). */
export function openSandbox(): VaultInfo {
  if (mockStatus.value.unlocked && !isSandbox()) throw refusal('opening the sandbox while a vault is open')
  keepPristine()
  if (!isSandbox()) aside = capture()
  restore(pristine ?? capture())
  const info = infoOf()
  show(info, true)
  return info
}

export function resetSandbox(): VaultInfo {
  if (!isSandbox()) throw refusal('resetting a sandbox that is not open')
  return openSandbox()
}

/** Leaves the sandbox: the real data come back, the unlock screen shows. `false` when it was not open. */
export function leaveSandbox(): boolean {
  if (!isSandbox()) return false
  if (aside) restore(aside)
  aside = null
  show(null, false)
  return true
}

/** What is switched off while the sandbox is open, as in `commands::sandbox::refuse`. */
const OFF_IN_SANDBOX: Record<string, string> = {
  transfer_receive_start: 'handing the vault over',
  transfer_browse_start: 'handing the vault over',
  transfer_send: 'handing the vault over',
  link_vault_external: 'an external copy',
  set_exchange_key: 'an exchange key',
  inspect_tx: 'looking up a transaction online',
  re_enrich: 'looking up a transaction online',
  datasource_preview: 'fetching a data source',
  datasource_test: 'fetching a data source',
  datasource_refresh: 'fetching a data source',
  send_test_notification: 'system notifications',
}

/** The refusal for `cmd` while in the sandbox, if it is off there. */
export function refusedInSandbox(cmd: string) {
  const what = OFF_IN_SANDBOX[cmd]
  return what && isSandbox() ? refusal(what) : undefined
}
