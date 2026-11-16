import type { RouteLocationRaw } from 'vue-router'

import { cryptoOpFilterToQuery } from '@/entities/crypto-tx'

/** The groups of the tab, in the order they are drawn. */
export const RELATION_GROUPS = ['own_addresses', 'exchanges', 'protocols'] as const
export type RelationGroup = (typeof RELATION_GROUPS)[number]

/** Where a row leads: the operations screen narrowed by the row's own filter, an open card when `op` is given. */
export function relationLocation(filter: CryptoOpFilter, op?: string): RouteLocationRaw {
  return { name: 'crypto-operations', query: { ...cryptoOpFilterToQuery(filter), op } }
}

export interface RelationSectionRow {
  key: string
  label: string
  address?: string
  count: number
  lastTime: string
  to: RouteLocationRaw
}

export interface RelationSection {
  group: RelationGroup
  rows: RelationSectionRow[]
  /** How many the group has in all: the answer is capped, and the screen names the rest. */
  total: number
}

/** The non-empty groups of the answer, each row already holding the place it opens. */
export function relationSections(data: CryptoRelations | undefined): RelationSection[] {
  if (!data) return []
  return RELATION_GROUPS.map((group) => ({
    group,
    total: data[group].total,
    rows: data[group].items.map((row) => ({
      key: row.key,
      label: row.label,
      address: row.address,
      count: row.count,
      lastTime: row.last_time,
      to: relationLocation(row.filter),
    })),
  })).filter((section) => section.rows.length > 0)
}

/** Whether the answer has nothing in any group. */
export function relationsEmpty(data: CryptoRelations | undefined): boolean {
  return !!data && RELATION_GROUPS.every((group) => data[group].items.length === 0) && data.fiat.items.length === 0
}
