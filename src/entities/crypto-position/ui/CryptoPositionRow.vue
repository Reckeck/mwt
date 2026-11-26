<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Layers } from 'lucide-vue-next'

import { formatDate } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { ListRow } from '@/shared/ui/list-row'

import type { CryptoPositionRowProps } from './crypto-position-row.types'

/**
 * One DeFi position as a list row: name, protocol and kind under it, what it is worth now at the end and the net
 * result under that (the `#net` slot). A figure nobody knows is a dash, never a zero; an approximate one carries «≈».
 */
const props = defineProps<CryptoPositionRowProps>()
defineSlots<{ value?(): unknown; net?(): unknown }>()
const { t } = useI18n()

const subtitle = computed(() => {
  const parts = [props.position.protocol, t(`cryptoTrack.positions.kind.${props.position.kind}`)]
  if (props.position.opened_at) parts.push(t('cryptoTrack.positions.opened', { date: formatDate(props.position.opened_at) }))
  return parts.join(' · ')
})
const closed = computed(() => props.position.status === 'closed')
const netKnown = computed(() => props.position.net_usd !== undefined && props.position.net_usd !== null)
</script>

<template>
  <ListRow :title="props.position.name" :subtitle="subtitle" :icon="Layers" data-position-row>
    <template #value>
      <div class="grid justify-items-end gap-0.5">
        <span class="flex items-center gap-1.5">
          <Badge v-if="closed" variant="neutral" data-position-closed>{{ t('cryptoTrack.positions.status.closed') }}</Badge>
          <span v-if="props.position.approximate" :title="t('cryptoTrack.positions.approximateMark')">≈</span>
          <span v-if="props.position.value_usd != null" data-position-value><slot name="value" /></span>
          <span v-else data-position-value-unknown>—</span>
        </span>
        <span class="text-xs font-normal text-muted-foreground" data-position-net>
          <slot v-if="netKnown" name="net" />
          <template v-else>{{ t('cryptoTrack.positions.netUnknown') }}</template>
        </span>
      </div>
    </template>
  </ListRow>
</template>
