/** Commands: developer tools, reports, losses, links and enrichment. */

import { txId, txMeta } from '@/shared/lib/transactions'
import {
  mockCalendar,
  mockMonthBalance,
  mockMonthPace,
  mockNetWorthTrend,
  mockOverview,
  mockReport,
  mockSpendingOverview,
} from './analytics'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockTransactions } from './demo'
import { mockScratchJson } from './exchange-data'
import { analyticsContext, mockCandles, mockFlows, mockFxTable, mockLossOf, mockSummary } from './market'
import { mockEntityCard } from './relations'
import { mockPinned, mockSearchIndex, mockSearchTransactions, mockSetPinned } from './search'
import { mockSettings, mockVaultDoc } from './vault-data'

/** Commands: developer tools, reports, losses, links and enrichment. */
export const devReportCommands: MockCommands = {
  dev_dump_vault: (_call) => {
    return resolve({ json: JSON.stringify(mockVaultDoc(), null, 2) })
  },
  dev_export_vault: (_call) => {
    // No native save dialog in the browser mock; report a plausible path so
    // the success path stays observable during `pnpm dev`.
    return resolve('/tmp/mock/vault.json')
  },
  dev_apply_vault_json: ({ args }) => {
    // Reject anything the editor would not accept, exactly like the backend.
    const text = String(args?.json ?? '')
    try {
      JSON.parse(text)
    } catch (err) {
      throw { code: 'DESERIALIZE', message: String(err) }
    }
    return resolve(JSON.stringify(JSON.parse(text), null, 2))
  },
  dev_open_vault_json_in_editor: (_call) => {
    return resolve('/tmp/tradeapp-vault.json')
  },
  dev_reload_vault_json_from_editor: (_call) => {
    // Stands in for the scratch file: whatever the backend last reported.
    return resolve(mockScratchJson)
  },
  get_candles: ({ body }) => {
    const input = body as GetCandlesInput
    return resolve(mockCandles(input))
  },
  get_report: ({ args }) => {
    return resolve(mockReport(args?.kind as ReportKind, args?.filter as ReportFilter, analyticsContext()))
  },
  get_report_overview: ({ args }) => {
    return resolve(mockOverview(args?.filter as ReportFilter, analyticsContext()))
  },
  get_calendar: ({ args }) => {
    return resolve(mockCalendar(String(args?.month ?? new Date().toISOString().slice(0, 7)), analyticsContext()))
  },
  get_month_balance: ({ args }) => {
    return resolve(mockMonthBalance(analyticsContext(), args?.filter as ReportFilter, mockSettings.carry_since))
  },
  set_carry_over: ({ args }) => {
    mockSettings.carry_since = (args?.month as string | null | undefined) ?? null
    return resolve(null)
  },
  get_spending_overview: ({ args }) => {
    const period = (args?.period as DashPeriod | undefined) ?? mockSettings.dashboard_period
    return resolve(mockSpendingOverview(period, analyticsContext()))
  },
  get_month_pace: (_call) => {
    return resolve(mockMonthPace(analyticsContext(), Number(mockSettings.savings_rate)))
  },
  get_net_worth_trend: ({ args }) => {
    const period = (args?.period as DashPeriod | undefined) ?? mockSettings.dashboard_period
    return resolve(mockNetWorthTrend(period, Number(mockSummary.total_base), analyticsContext().currency))
  },
  get_entity_card: ({ args }) => {
    return resolve(mockEntityCard(args?.target as EntityRef))
  },
  get_search_index: (_call) => {
    return resolve(mockSearchIndex())
  },
  search_transactions: ({ args }) => {
    const categories = (args?.categories as string[] | undefined) ?? []
    return resolve(mockSearchTransactions(String(args?.query ?? ''), categories, Number(args?.limit ?? 20)))
  },
  get_pinned: (_call) => {
    return resolve(mockPinned())
  },
  set_pinned: ({ args }) => {
    return resolve(mockSetPinned((args?.pins as Pinned[] | undefined) ?? []))
  },
  get_fx_table: (_call) => {
    return resolve(mockFxTable())
  },
  get_tx_loss: ({ args }) => {
    return resolve(mockLossOf(String(args?.id)))
  },
  get_tx_losses: (_call) => {
    return resolve(Object.fromEntries(mockTransactions.map((tx) => [txId(tx), mockLossOf(txId(tx))])))
  },
  list_flows: (_call) => {
    return resolve(mockFlows())
  },
  link_transactions: ({ body }) => {
    const input = body as LinkTransactionsInput
    const flow =
      input.flow_id ??
      mockTransactions.map((tx) => (input.ids.includes(txId(tx)) ? txMeta(tx).flow_id : null)).find(Boolean) ??
      crypto.randomUUID()
    for (const tx of mockTransactions) if (input.ids.includes(txId(tx))) txMeta(tx).flow_id = flow
    return resolve(flow)
  },
  unlink_transaction: ({ args }) => {
    const tx = mockTransactions.find((entry) => txId(entry) === args?.id)
    if (tx) txMeta(tx).flow_id = null
    return resolve({})
  },
  get_enrichments: (_call) => {
    return resolve([])
  },
  re_enrich: (_call) => {
    return resolve(null)
  },
  apply_enrichment: (_call) => {
    return Promise.reject({ code: 'MISSING', message: 'mock: no findings in the browser' })
  },
  reject_enrichment: (_call) => {
    return Promise.reject({ code: 'MISSING', message: 'mock: no findings in the browser' })
  },
  export_report_csv: (_call) => {
    // No save dialog in the browser.
    return resolve(null)
  },
  export_transactions_csv: (_call) => resolve(null),
}
