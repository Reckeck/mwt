<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { formatDay } from '@/shared/lib/format'

/**
 * «Looks like an exchange»: the app noticed a fiat operation that fits an incoming one and only says so.
 * Nothing is linked until the user presses `Link`. `Not an exchange` is the user's answer about their own
 * data, so it is written to the vault and travels with it — it is not forgotten by the next device or the
 * next install, and the guess is never offered again.
 */
const props = defineProps<{ suggestion: CryptoExchangeSuggestion; busy?: boolean }>()
const emit = defineEmits<{ link: []; dismiss: [] }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-primary/30 bg-primary/10 p-4" data-crypto-exchange-banner>
    <div class="min-w-0 flex-1 basis-64">
      <p class="font-medium">{{ t('cryptoTrack.operations.banner.title') }}</p>
      <p class="text-sm text-muted-foreground">
        <i18n-t keypath="cryptoTrack.operations.banner.body" tag="span">
          <template #date>{{ formatDay(props.suggestion.candidate.time) }}</template>
          <template #place>{{ props.suggestion.candidate.place }}</template>
          <template #spent><Money :amount="props.suggestion.candidate.amount" :currency="props.suggestion.candidate.currency" :convert="false" /></template>
          <template #got><Money :amount="props.suggestion.amount" :currency="props.suggestion.symbol" signed :convert="false" /></template>
        </i18n-t>
      </p>
    </div>
    <div class="flex shrink-0 gap-2">
      <Button variant="outline" :disabled="props.busy" @click="emit('dismiss')">{{ t('cryptoTrack.operations.banner.no') }}</Button>
      <Button @click="emit('link')">{{ t('cryptoTrack.operations.banner.yes') }}</Button>
    </div>
  </div>
</template>
