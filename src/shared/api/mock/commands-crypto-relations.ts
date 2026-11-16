/**
 * Commands: who an address dealt with (spec E15).
 *
 * Same shape as the Rust service, built from the mock's own operations: the user's other addresses and the
 * protocols are read off the rows, and the fiat records come from the pointers of spec E13. Nothing is stored.
 */

import { type MockCommands, resolve } from './core'
import { cryptoLinksCommands } from './commands-crypto-links'
import { cryptoOperationsCommands } from './commands-crypto-operations'

const call = (cmd: string, body: Record<string, unknown>) => ({ cmd, args: { input: body }, body })

export const cryptoRelationsCommands: MockCommands = {
  crypto_relations: async ({ body }) => {
    const { account_id } = body as unknown as CryptoRelationsInput
    const filter: CryptoOpFilter = { account_ids: [account_id], show_hidden: false }
    const page = (await cryptoOperationsCommands.crypto_list_operations(
      call('crypto_list_operations', { filter, offset: 0, limit: 100 }),
    )) as CryptoOpsPage
    const rows = page.items.filter((row) => !row.own_entry)

    const protocols = new Map<string, CryptoRelationRow>()
    for (const row of rows) {
      if (!row.protocol) continue
      const have = protocols.get(row.protocol)
      protocols.set(row.protocol, {
        key: row.protocol,
        label: row.protocol,
        count: (have?.count ?? 0) + 1,
        last_time: have && have.last_time > row.time ? have.last_time : row.time,
        filter: { ...filter, protocol: row.protocol },
      })
    }

    const pairs = (await cryptoLinksCommands.crypto_link_pairs(call('crypto_link_pairs', {}))) as CryptoPair[]
    const fiat: CryptoFiatRelation[] = pairs.flatMap((pair) =>
      pair.event?.account_id === account_id && pair.fiat ? [{ event_id: pair.event_id, fiat: pair.fiat, filter }] : [],
    )

    const group = <Row>(items: Row[]) => ({ items, total: items.length })
    const found: CryptoRelations = {
      own_addresses: group([]),
      exchanges: group([]),
      protocols: group([...protocols.values()].sort((left, right) => right.count - left.count)),
      fiat: group(fiat),
    }
    return resolve(found)
  },
}
