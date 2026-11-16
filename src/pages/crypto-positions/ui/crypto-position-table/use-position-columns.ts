import { computed, h } from 'vue'
import { useI18n } from 'vue-i18n'

import { PnlText } from '@/entities/currency'
import { ActionMenu } from '@/shared/ui/action-menu'
import type { ColumnDef } from '@/shared/ui/data-table'

import { accrualFigure, feesFigure, protocolLabel, unpricedSymbols, valueFigure } from '@/entities/crypto-position'
import { PositionFigure } from '../position-figure'
import { PositionRate } from '../position-rate'
import PositionBadges from './PositionBadges.vue'
import type { usePositionActions } from './use-position-actions'

const num = (value: string | undefined) => (value === undefined ? Number.NEGATIVE_INFINITY : Number(value))

/** Columns of the position table: the position, put in, worth now, accrual, fees, net, rate, and the «⋯». */
export function usePositionColumns(actionsOf: ReturnType<typeof usePositionActions>) {
  const { t } = useI18n()
  const figure = (
    position: CryptoPositionView,
    amount: string | null | undefined,
    extra: Record<string, unknown> = {},
  ) => h(PositionFigure, { amount, approximate: position.approximate, ...extra })

  return computed<ColumnDef<CryptoPositionView>[]>(() => [
    {
      id: 'position',
      header: t('cryptoTrack.positions.col.position'),
      accessorFn: (row) => row.name,
      cell: ({ row }) =>
        h('div', { class: 'flex min-w-0 flex-col gap-1' }, [
          h(
            'span',
            { class: 'truncate font-medium' },
            `${protocolLabel(row.original.protocol)} · ${row.original.name}`,
          ),
          h(PositionBadges, { position: row.original }),
        ]),
    },
    {
      id: 'putIn',
      header: t('cryptoTrack.positions.putIn'),
      accessorFn: (row) => num(row.put_in_usd),
      cell: ({ row }) => h(PositionFigure, { amount: row.original.put_in_usd, reason: 'noHistory' }),
      meta: { align: 'right' },
    },
    {
      id: 'value',
      header: t('cryptoTrack.positions.col.value'),
      accessorFn: (row) => num(row.value_usd),
      cell: ({ row }) => {
        const value = valueFigure(row.original)
        return figure(row.original, value.amount, { reason: value.reason, tokens: unpriced(row.original) })
      },
      meta: { align: 'right' },
    },
    {
      id: 'accrual',
      header: () =>
        h('span', { title: t('cryptoTrack.positions.breakdown.noteAccrues') }, t('cryptoTrack.positions.col.accrual')),
      enableSorting: false,
      cell: ({ row }) => {
        const accrual = accrualFigure(row.original)
        return figure(row.original, accrual.amount, { reason: accrual.reason, tokens: unpriced(row.original) })
      },
      meta: { align: 'right' },
    },
    {
      id: 'fees',
      header: () => h('span', { 'data-tour': 'defi-fees' }, t('cryptoTrack.positions.col.fees')),
      enableSorting: false,
      cell: ({ row }) => {
        const fees = feesFigure(row.original)
        return figure(row.original, fees.amount, { reason: fees.reason })
      },
      meta: { align: 'right' },
    },
    {
      id: 'net',
      header: t('cryptoTrack.positions.net'),
      accessorFn: (row) => num(row.net_usd),
      cell: ({ row }) =>
        row.original.net_usd === undefined
          ? h(PositionFigure, {
              amount: null,
              reason: unpriced(row.original) ? 'noPrice' : 'notRead',
              tokens: unpriced(row.original),
            })
          : h('span', { class: 'inline-flex items-baseline gap-[0.25em]' }, [
              row.original.approximate ? h('span', { class: 'text-muted-foreground' }, '≈') : null,
              h(PnlText, { amount: row.original.net_usd, currency: 'usd', decimals: 2 }),
            ]),
      meta: { align: 'right', class: 'font-medium' },
    },
    {
      id: 'apr',
      header: 'APR',
      accessorFn: (row) => num(row.apr),
      cell: ({ row }) => h(PositionRate, { apr: row.original.apr, approximate: row.original.approximate }),
      meta: { align: 'right' },
    },
    {
      id: 'actions',
      header: '',
      enableSorting: false,
      // The place for the «⋯» is always there, so nothing moves when the pointer arrives (rule 17).
      // The menu's clicks are its own: they must not also open the row.
      cell: ({ row }) =>
        h(
          'span',
          {
            class: 'inline-flex',
            onClick: (event: Event) => event.stopPropagation(),
            onKeydown: (event: Event) => event.stopPropagation(),
          },
          [h(ActionMenu, { actions: actionsOf(row.original), label: t('cryptoTrack.positions.actions.label') })],
        ),
      meta: { align: 'right', class: 'w-10' },
    },
  ])
}

const unpriced = (position: CryptoPositionView): string => unpricedSymbols(position).join(', ')
