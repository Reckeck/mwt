<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { ListRow } from '@/shared/ui/list-row'
import { formatDateTime } from '@/shared/lib/format'

import { cryptoOpIcon, legAmount, opPlace, opTitle, shortAddress } from '../lib/op-view'

/**
 * One operation of the module as a row of the shared list. A tap opens the chain under it: every leg
 * of the exchange or swap, the two sides, the reference and the fee. Nothing here edits an amount:
 * they come from the network and are shown as they are.
 */
const props = defineProps<{ row: CryptoOpRow }>()
const { t } = useI18n()

const title = computed(() => opTitle(props.row, t))
const incoming = computed(() => props.row.legs[0]?.direction === 'in')
/** The other side: the contact when one is known, else its address, with the direction's word. */
const other = computed(() => {
  const address = incoming.value ? props.row.from : props.row.to
  if (props.row.contact_name) return props.row.contact_name
  return address ? t(incoming.value ? 'cryptoTrack.operations.from' : 'cryptoTrack.operations.to', { address: shortAddress(address) }) : ''
})
const subtitle = computed(() => [other.value, opPlace(props.row)].filter(Boolean).join(' · '))
const reference = computed(() =>
  props.row.reference ? t(props.row.exchange ? 'cryptoTrack.operations.fill' : 'cryptoTrack.operations.hash', { id: props.row.reference }) : '',
)
</script>

<template>
  <Collapsible v-slot="{ open }" :data-crypto-op="props.row.id">
    <CollapsibleTrigger class="flex w-full items-center gap-2 px-1 text-left" data-crypto-op-open>
      <ListRow class="min-w-0 flex-1" :title="title" :subtitle="subtitle" :icon="cryptoOpIcon(props.row)">
        <template #value>
          <span v-for="leg in props.row.legs" :key="leg.asset_key + leg.direction" class="block" :class="leg.direction === 'in' ? 'text-success' : 'text-destructive'">
            <Money :amount="legAmount(leg)" :currency="leg.symbol" :at="props.row.time" signed :convert="false" />
          </span>
          <span v-if="props.row.value_usd" class="block text-xs font-normal text-muted-foreground">
            <Money :amount="props.row.value_usd" currency="usd" signed />
          </span>
        </template>
      </ListRow>
      <ChevronDown class="size-4 shrink-0 text-muted-foreground transition-transform" :class="open && 'rotate-180'" aria-hidden="true" />
      <span class="sr-only">{{ t('cryptoTrack.operations.open') }}</span>
    </CollapsibleTrigger>
    <CollapsibleContent>
      <div class="grid gap-1 px-3 pb-3 text-xs text-muted-foreground" data-crypto-op-chain>
        <div class="flex flex-wrap items-center gap-2">
          <Badge variant="outline">{{ t(`cryptoTrack.operations.kind.${props.row.kind}`) }}</Badge>
          <Badge v-if="props.row.flow_id" variant="outline" data-crypto-op-exchange>{{ t('cryptoTrack.operations.exchange') }}</Badge>
          <Badge v-if="props.row.hidden" variant="secondary">{{ t('cryptoTrack.operations.hiddenBadge') }}</Badge>
          <span>{{ formatDateTime(props.row.time) }}</span>
        </div>
        <p v-for="leg in props.row.legs" :key="`${leg.asset_key}-${leg.direction}`" class="tabular-nums">
          <Money :amount="legAmount(leg)" :currency="leg.symbol" :at="props.row.time" signed :convert="false" />
        </p>
        <p v-if="props.row.from || props.row.to" class="break-all">{{ shortAddress(props.row.from) }} → {{ shortAddress(props.row.to) }}</p>
        <p v-if="reference" class="break-all">{{ reference }}</p>
        <p v-if="props.row.protocol || props.row.comment">{{ [props.row.protocol, props.row.comment].filter(Boolean).join(' · ') }}</p>
        <p v-if="props.row.gas && props.row.gas_symbol" class="tabular-nums">
          {{ t('cryptoTrack.operations.gas') }} <Money :amount="props.row.gas.amount" :currency="props.row.gas_symbol" :convert="false" />
        </p>
      </div>
    </CollapsibleContent>
  </Collapsible>
</template>
