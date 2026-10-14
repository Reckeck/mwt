/** Commands: developer tools, reports, losses, links and enrichment. */

import { txId, txMeta } from '@/shared/lib/transactions'
import { mockCalendar, mockReport } from './analytics'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockTransactions } from './demo'
import { mockScratchJson } from './exchange-data'
import { analyticsContext, mockCandles, mockFlows, mockFxTable, mockLossOf } from './market'
import { mockVaultDoc } from './vault-data'

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
  get_calendar: ({ args }) => {
    return resolve(mockCalendar(String(args?.month ?? new Date().toISOString().slice(0, 7)), analyticsContext()))
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
}
