<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatAsOf } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { Badge } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'

import { DEFAULT_SOURCE_LABEL, networkLabel } from '../crypto-account.constants'
import { shortHex } from '@/shared/lib/short-hex'

/**
 * A followed address: its name, the network, the read-only mark, the address in monospace, when it was last read
 * and from where. The right side is a slot (the owner puts the value or the settings entry there), so money
 * stays a `<Money>` of the page.
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

const updated = computed(() =>
  props.readAt ? t('cryptoTrack.accounts.updated', { time: formatAsOf(props.readAt) }) : t('cryptoTrack.accounts.notRead'),
)
const source = computed(() => props.source ?? DEFAULT_SOURCE_LABEL)
</script>

<template>
  <Card class="flex flex-wrap items-start justify-between gap-x-6 gap-y-3 p-4 md:p-6" data-tour="crypto-overview" data-crypto-address-card>
    <div class="flex min-w-0 flex-col gap-1.5">
      <div class="flex flex-wrap items-center gap-2">
        <span class="text-lg font-semibold">{{ props.name }}</span>
        <Badge variant="secondary">{{ networkLabel(props.chain) }}</Badge>
        <Badge variant="outline">{{ t('cryptoTrack.accounts.readOnly') }}</Badge>
      </div>
      <span class="font-mono text-sm break-all text-muted-foreground" data-crypto-address :title="props.address">{{
        phone ? shortHex(props.address) : props.address
      }}</span>
      <span class="text-xs text-muted-foreground">{{ updated }} · {{ t('cryptoTrack.accounts.source', { source }) }}</span>
    </div>
    <div class="flex flex-col items-end gap-0.5 tabular-nums max-md:items-start"><slot /></div>
  </Card>
</template>
