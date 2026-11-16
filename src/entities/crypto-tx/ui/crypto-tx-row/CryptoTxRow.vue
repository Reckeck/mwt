<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency/@x/crypto-tx'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { formatDateTime } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'

import { cryptoOpIcon, legAmount, opPlace, opTitle, shortAddress } from '../../lib/op-view'
import type { CryptoTxRowProps } from './crypto-tx-row.types'

/**
 * One operation of the module, in the anatomy of the app's own operations list: an icon cell, a text cell
 * (title, who and where, the reference), an amount stack and a fixed column for the row's menu (`#actions`).
 * Nothing here edits an amount: they come from the network and are shown as they are.
 */
const props = withDefaults(defineProps<CryptoTxRowProps>(), { openable: false })
const emit = defineEmits<{ open: [row: CryptoOpRow] }>()
const slots = defineSlots<{ actions?(props: { row: CryptoOpRow }): unknown }>()

const { t } = useI18n()
const phone = useIsPhone()

const title = computed(() => opTitle(props.row, t))
const icon = computed(() => cryptoOpIcon(props.row))
const incoming = computed(() => props.row.legs[0]?.direction === 'in')
/** The other side: the contact when one is known, else its address, with the direction's word. */
const other = computed(() => {
  const address = incoming.value ? props.row.from : props.row.to
  const name = props.row.contact_name
  if (name) return name
  return address ? t(incoming.value ? 'cryptoTrack.operations.from' : 'cryptoTrack.operations.to', { address: shortAddress(address) }) : ''
})
const where = computed(() => [other.value, opPlace(props.row)].filter(Boolean).join(' · '))
const reference = computed(() =>
  props.row.reference ? t(props.row.exchange ? 'cryptoTrack.operations.fill' : 'cryptoTrack.operations.hash', { id: props.row.reference }) : '',
)
const note = computed(() => props.row.comment || (props.row.protocol ? props.row.protocol : ''))
</script>

<template>
  <div class="flex gap-x-3 gap-y-1 py-3" :class="phone ? 'items-center' : 'flex-wrap items-start'" :data-crypto-op="props.row.id">
    <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground" data-tx-kind>
      <component :is="icon" class="size-4" aria-hidden="true" />
      <span class="sr-only">{{ t(`cryptoTrack.operations.kind.${props.row.kind}`) }}</span>
    </span>

    <div class="relative min-w-0 flex-1" :class="!phone && 'basis-56'">
      <Button
        v-if="props.openable"
        variant="ghost"
        size="block"
        class="absolute inset-0 rounded-sm p-0 hover:bg-transparent"
        :title="t('cryptoTrack.operations.open')"
        :aria-label="t('cryptoTrack.operations.open')"
        data-crypto-op-open
        @click="emit('open', props.row)"
      />
      <div class="pointer-events-none relative flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span class="font-medium" data-tx-title>{{ title }}</span>
        <Badge v-if="props.row.flow_id" variant="outline" class="shrink-0" data-crypto-op-exchange>{{ t('cryptoTrack.operations.exchange') }}</Badge>
        <Badge v-if="props.row.own_entry" variant="secondary" class="shrink-0" data-crypto-own-entry>
          {{ t(`cryptoTrack.operations.own.${props.row.own_entry}`) }}
        </Badge>
        <Badge v-if="props.row.hidden" variant="secondary" class="shrink-0">{{ t('cryptoTrack.operations.hiddenBadge') }}</Badge>
        <Badge v-else-if="props.row.by_rule" variant="outline" class="shrink-0">{{ t('cryptoTrack.operations.byRuleBadge') }}</Badge>
        <Badge v-else-if="props.row.corrected && !props.row.flow_id" variant="outline" class="shrink-0">{{ t('cryptoTrack.operations.correctedBadge') }}</Badge>
      </div>
      <p class="pointer-events-none relative truncate text-xs text-muted-foreground">
        {{ where }}
      </p>
      <p v-if="!phone && (note || reference)" class="pointer-events-none relative truncate text-xs text-muted-foreground">
        {{ [note, reference].filter(Boolean).join(' · ') }}
      </p>
      <p v-else-if="phone" class="pointer-events-none relative truncate text-xs text-muted-foreground">
        {{ [formatDateTime(props.row.time), note].filter(Boolean).join(' · ') }}
      </p>
    </div>

    <div class="ml-auto shrink-0 text-right">
      <p
        v-for="leg in props.row.legs"
        :key="leg.asset_key + leg.direction"
        class="whitespace-nowrap font-medium tabular-nums"
        :class="leg.direction === 'in' ? 'text-success' : 'text-destructive'"
      >
        <Money :amount="legAmount(leg)" :currency="leg.symbol" :at="props.row.time" signed :convert="false" />
      </p>
      <p v-if="props.row.gas && props.row.gas_symbol" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
        {{ t('cryptoTrack.operations.gas') }} <Money :amount="props.row.gas.amount" :currency="props.row.gas_symbol" :convert="false" />
      </p>
      <p v-if="props.row.value_usd" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
        <Money :amount="props.row.value_usd" currency="usd" signed />
      </p>
    </div>

    <!-- One column of fixed width in every row: nothing moves on hover, the amounts line up. -->
    <div class="flex w-8 shrink-0 justify-end">
      <slot v-if="slots.actions" name="actions" :row="props.row" />
    </div>
  </div>
</template>
