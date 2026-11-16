<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { formatAsOf } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'

import { DEFAULT_SOURCE_LABEL, networkLabel } from '../../crypto-account.constants'
import { shortAddress } from '../../lib/address'

/**
 * The top of an address wallet (mockups 01 and 02): its name, the network, the read-only mark, the
 * address in monospace, when it was last read and from where. The address's value goes in the
 * default slot so it stays a `<Money>` owned by the page.
 */
const props = defineProps<{
  name: string
  chain: Chain
  address: string
  /** The source that reads it; `null` is the network's public explorer. */
  source: string | null
  /** When the last read finished (RFC3339); `null` before the first one. */
  readAt: string | null
}>()
const { t } = useI18n()
const phone = useIsPhone()

const updated = computed(() => (props.readAt ? t('cryptoTrack.accounts.updated', { time: formatAsOf(props.readAt) }) : t('cryptoTrack.accounts.notRead')))
const source = computed(() => props.source ?? DEFAULT_SOURCE_LABEL)
</script>

<template>
  <div class="flex flex-wrap items-start justify-between gap-x-6 gap-y-3 rounded-xl border border-border bg-card p-4 shadow-sm" data-tour="crypto-overview">
    <div class="flex min-w-0 flex-col gap-1.5">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-lg font-semibold">{{ props.name }}</span>
        <Badge variant="secondary">{{ networkLabel(props.chain) }}</Badge>
        <Badge variant="outline">{{ t('cryptoTrack.accounts.readOnly') }}</Badge>
      </div>
      <span class="font-mono text-sm break-all text-muted-foreground" data-crypto-address :title="props.address">{{ phone ? shortAddress(props.address) : props.address }}</span>
      <span class="text-caption text-muted-foreground">{{ updated }} · {{ t('cryptoTrack.accounts.source', { source }) }}</span>
    </div>
    <div class="flex flex-col items-end gap-0.5 tabular-nums max-md:items-start">
      <span class="text-caption text-muted-foreground md:hidden">{{ t('cryptoTrack.accounts.value') }}</span>
      <slot />
    </div>
  </div>
</template>
