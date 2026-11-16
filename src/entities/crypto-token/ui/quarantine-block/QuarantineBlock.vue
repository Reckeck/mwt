<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { formatDate, formatQuantity } from '@/shared/lib/format'

import type { QuarantineRow } from '../../crypto-token.types'
import { shortContract } from '../../lib/short-contract'

/**
 * The tokens that came to an address by themselves and wait for a decision (mockups 03 to 05): not
 * in the address's value, its profit or the reports until the user recognises them. A token's own
 * text is plain text from a chain, so it is only ever interpolated, never rendered as markup.
 */
const props = defineProps<{ rows: QuarantineRow[]; busy?: boolean }>()
const emit = defineEmits<{ recognize: [row: QuarantineRow]; hide: [row: QuarantineRow]; why: [] }>()
const { t } = useI18n()

/** Why this one looks off, when the backend said so. */
function reason(row: QuarantineRow): string {
  if (row.warnings.includes('imitates_known')) return t('cryptoTrack.quarantine.imitates')
  if (row.warnings.includes('look_alike')) return t('cryptoTrack.quarantine.lookAlike')
  return ''
}
</script>

<template>
  <section class="flex flex-col gap-3 rounded-xl border border-warning bg-card p-4 shadow-sm" data-tour="crypto-quarantine" data-crypto-quarantine>
    <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
      <Badge variant="warning">{{ t('cryptoTrack.quarantine.badge') }}</Badge>
      <h3 class="text-base font-semibold">{{ t('cryptoTrack.quarantine.title', { count: props.rows.length }) }}</h3>
      <Button variant="link" class="touch-hit-xl ml-auto h-auto p-0" @click="emit('why')">{{ t('cryptoTrack.quarantine.why') }}</Button>
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
          <span class="text-caption break-words text-muted-foreground">
            {{ t('cryptoTrack.quarantine.meta', { contract: shortContract(row.contract), date: formatDate(row.firstSeen) }) }}
            <template v-if="reason(row)"> · {{ reason(row) }}</template>
          </span>
        </div>
        <div class="flex shrink-0 gap-2 max-md:w-full">
          <Button variant="outline" class="max-md:h-11 max-md:flex-1" :disabled="props.busy" @click="emit('hide', row)">
            {{ t('cryptoTrack.quarantine.hide') }}
          </Button>
          <Button class="max-md:h-11 max-md:flex-1" :disabled="props.busy" @click="emit('recognize', row)">
            {{ t('cryptoTrack.quarantine.recognize') }}
          </Button>
        </div>
      </li>
    </ul>
  </section>
</template>
