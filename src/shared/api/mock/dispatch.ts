/** The entry point: finds the command, or hands it to planning and workflows. */

import { mockCategories, mockTransactionFrom } from './ledger'
import { mockPlanning } from './planning'
import { mockWorkflows } from './workflows'
import { appSettingsOutsideCommands, notificationsCommands, vaultCommands } from './commands-app'
import { devReportCommands } from './commands-dev'
import { bybitImportStagingCommands, exchangeAccountsCommands, exchangeReadOnlyCommands } from './commands-exchange'
import { attachmentCommands } from './commands-attachments'
import { bulkCommands } from './commands-bulk'
import { contactsCommands, transactionsCommands } from './commands-ledger'
import { loyaltyDepositCommands } from './commands-loyalty'
import { sharedEventCommands } from './commands-shared-events'
import { sharedMoneyCommands } from './commands-shared-money'
import { sharedSettleCommands } from './commands-shared-settle'
import { ratesCommands } from './commands-rates'
import { updatesCommands } from './commands-updates'
import { sourcesCommands } from './commands-sources'
import { storeTreeCommands } from './commands-store-tree'
import { valuationCommands } from './commands-valuation'
import { vehicleCommands } from './commands-vehicle'
import { syncCommands } from './commands-sync'
import { transferCommands } from './commands-transfer'
import { vaultBackupsCommands } from './commands-vault-backups'
import { vaultFilesCommands } from './commands-vault-files'
import { vaultPasswordCommands } from './commands-vault-password'
import { overviewCommands, walletsCommands } from './commands-wallets'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockStores, mockTransactions, priceOf } from './demo'
import { sandboxCommands } from './commands-sandbox'
import { keepPristine, refusedInSandbox } from './sandbox-state'
import { seedMock } from './scenario-seed'
import { mockAppSettings, mockSettings } from './vault-data'
import { mockScenario } from './scenario'

/** Every command the browser mock answers, by name. */
const MOCK_COMMANDS: MockCommands = {
  ...vaultCommands,
  ...vaultFilesCommands,
  ...vaultBackupsCommands,
  ...vaultPasswordCommands,
  ...sandboxCommands,
  ...syncCommands,
  ...transferCommands,
  ...notificationsCommands,
  ...appSettingsOutsideCommands,
  ...contactsCommands,
  ...transactionsCommands,
  ...bulkCommands,
  ...attachmentCommands,
  ...exchangeReadOnlyCommands,
  ...bybitImportStagingCommands,
  ...walletsCommands,
  ...storeTreeCommands,
  ...vehicleCommands,
  ...valuationCommands,
  ...overviewCommands,
  ...loyaltyDepositCommands,
  ...sharedEventCommands,
  ...sharedSettleCommands,
  ...sharedMoneyCommands,
  ...ratesCommands,
  ...updatesCommands,
  ...sourcesCommands,
  ...exchangeAccountsCommands,
  ...devReportCommands,
}

/** What still answers in the `error` scenario: enough to unlock and reach the screen. */
const BOOT_COMMANDS = new Set(
  Object.keys({ ...vaultCommands, ...vaultFilesCommands, ...appSettingsOutsideCommands, ...notificationsCommands }),
)

export function mock(cmd: string, rawArgs?: Record<string, unknown>): Promise<unknown> {
  seedMock()
  keepPristine()
  if (mockScenario === 'error' && !BOOT_COMMANDS.has(cmd)) {
    return new Promise((_, fail) =>
      setTimeout(() => fail({ code: 'MOCK_ERROR', message: `Сценарий error: ${cmd} отказал` }), 30),
    )
  }
  // Like Tauri, which sends arguments as JSON: detaches Vue proxies the mock would otherwise store.
  const args = rawArgs ? (JSON.parse(JSON.stringify(rawArgs)) as Record<string, unknown>) : rawArgs
  const body = args?.input as Record<string, unknown> | undefined
  const refused = refusedInSandbox(cmd)
  if (refused) return Promise.reject(refused)
  const handler = MOCK_COMMANDS[cmd]
  if (handler) return handler({ cmd, args, body })
  try {
    const flowed = mockWorkflows(cmd, args)
    if (flowed !== undefined) return resolve(flowed)
  } catch (err) {
    return Promise.reject(err)
  }
  const planned = mockPlanning(
    cmd,
    args,
    {
      transactions: mockTransactions,
      categories: mockCategories,
      stores: mockStores,
      priceOf,
      transactionFrom: (input, ts) =>
        mockTransactionFrom({ ...input, common: { ...input.common, timestamp: ts } } as TransactionInput, priceOf),
      budgetCurrency: mockSettings.budget_currency || mockAppSettings.base_currency,
    },
    mockSettings,
  )
  return resolve(planned ?? {})
}
