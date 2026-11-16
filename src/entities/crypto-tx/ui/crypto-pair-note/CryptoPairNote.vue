<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency/@x/crypto-tx'
import { formatDay } from '@/shared/lib/format'

import { CryptoTxRow } from '../crypto-tx-row'
import type { CryptoPairNoteProps } from './crypto-pair-note.types'

/**
 * The other end of a pointer between a crypto operation and a fiat record (spec E13), shown from either side.
 * A pointer is a reference only: nothing here is a sum that counts. An end that is gone is a calm sentence,
 * never an error: the record was deleted, or the module was wiped.
 */
const props = defineProps<CryptoPairNoteProps>()
const { t } = useI18n()
</script>

<template>
  <div data-crypto-pair-note>
    <template v-if="props.show === 'fiat'">
      <div v-if="props.pair.fiat" class="flex items-center justify-between gap-3 rounded-lg border px-3 py-2">
        <span class="min-w-0">
          <span class="block truncate text-sm font-medium">{{ props.pair.fiat.comment || props.pair.fiat.place }}</span>
          <span class="block truncate text-xs text-muted-foreground">
            {{ [props.pair.fiat.place, formatDay(props.pair.fiat.time)].filter(Boolean).join(' · ') }}
          </span>
        </span>
        <span class="shrink-0 font-medium tabular-nums">
          <Money :amount="props.pair.fiat.amount" :currency="props.pair.fiat.currency" :at="props.pair.fiat.time" signed />
        </span>
      </div>
      <p v-else class="rounded-lg bg-muted p-3 text-sm text-muted-foreground" data-crypto-pair-missing>
        {{ t('cryptoTrack.links.recordMissing') }}
      </p>
    </template>
    <template v-else>
      <div v-if="props.pair.event" class="rounded-lg border px-3"><CryptoTxRow :row="props.pair.event" /></div>
      <p v-else class="rounded-lg bg-muted p-3 text-sm text-muted-foreground" data-crypto-pair-missing>
        {{ t('cryptoTrack.links.operationMissing') }}
      </p>
    </template>
  </div>
</template>
