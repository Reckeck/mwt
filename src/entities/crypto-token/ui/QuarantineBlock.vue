<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { formatDate, formatQuantity } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

import type { QuarantineRow } from '../crypto-token.types'
import { shortHex } from '@/shared/lib/short-hex'

/**
 * The tokens that came to an address by themselves and wait for a decision: not in the address's value, its
 * profit or the reports until the user recognises them. A token's own text is plain text from a chain, so it is
 * only ever interpolated, never rendered as markup.
 */
const props = defineProps<{ rows: QuarantineRow[]; busy?: boolean }>()
const emit = defineEmits<{ recognize: [row: QuarantineRow]; hide: [row: QuarantineRow] }>()
const { t } = useI18n()

/** Why this one looks off, when the backend said so. */
function reason(row: QuarantineRow): string {
  if (row.warnings.includes('imitates_known')) return t('cryptoTrack.quarantine.imitates')
  if (row.warnings.includes('look_alike')) return t('cryptoTrack.quarantine.lookAlike')
  return ''
}
</script>

<template>
  <Card class="flex flex-col gap-3 border-warning p-4 md:p-6" data-tour="crypto-quarantine" data-crypto-quarantine>
    <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
      <Badge variant="warning">{{ t('cryptoTrack.quarantine.badge') }}</Badge>
      <h3 class="text-base font-semibold">{{ t('cryptoTrack.quarantine.title', { count: props.rows.length }) }}</h3>
    </div>
    <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.quarantine.lead') }}</p>
    <ul class="flex flex-col">
      <li
        v-for="row in props.rows"
        :key="row.id"
        class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t border-border py-3 first:border-t-0"
        data-crypto-quarantine-row
      >
        <div class="flex min-w-0 flex-col gap-0.5">
          <span class="truncate font-medium">{{ row.symbol }} · {{ formatQuantity(row.quantity, 2) }}</span>
          <span class="break-words text-xs text-muted-foreground">
            {{ t('cryptoTrack.quarantine.meta', { contract: shortHex(row.contract), date: formatDate(row.firstSeen) }) }}
            <template v-if="reason(row)"> · {{ reason(row) }}</template>
          </span>
        </div>
        <div class="flex shrink-0 gap-2 max-md:w-full">
          <Button variant="outline" class="max-md:h-11 max-md:flex-1" :disabled="props.busy" data-quarantine-hide @click="emit('hide', row)">
            {{ t('cryptoTrack.quarantine.hide') }}
          </Button>
          <Button class="max-md:h-11 max-md:flex-1" :disabled="props.busy" data-quarantine-recognize @click="emit('recognize', row)">
            {{ t('cryptoTrack.quarantine.recognize') }}
          </Button>
        </div>
      </li>
    </ul>
  </Card>
</template>
