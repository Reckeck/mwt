<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowRight } from 'lucide-vue-next'

import { Money, PnlText } from '@/entities/currency'
import { decimalSub } from '@/shared/lib/decimal'

import type { SyncAfterProps } from './sync-after.types'

/** Capital and the balances of the wallets a batch moves, before and after it is applied. */
const props = defineProps<SyncAfterProps>()
const { t } = useI18n()

const delta = computed(() => decimalSub(props.after.net_worth_after, props.after.net_worth_before))
</script>

<template>
  <section class="grid gap-1.5 rounded-md border border-border bg-muted/30 px-3 py-2 text-sm" data-tour="sync-after">
    <p class="font-medium">{{ t('sync.after.title') }}</p>
    <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
      <span class="min-w-0 flex-1 truncate">{{ t('sync.after.netWorth') }}</span>
      <Money :amount="props.after.net_worth_before" :currency="props.after.currency" class="text-muted-foreground" />
      <ArrowRight class="size-3.5 shrink-0 self-center text-muted-foreground" aria-hidden="true" />
      <Money :amount="props.after.net_worth_after" :currency="props.after.currency" class="font-medium" />
      <PnlText v-if="delta" :amount="delta" :currency="props.after.currency" class="text-xs" />
    </div>
    <ul v-if="props.after.stores.length" class="grid gap-0.5 text-xs">
      <li v-for="store in props.after.stores" :key="store.id" class="flex flex-wrap items-baseline gap-x-2">
        <span class="min-w-0 flex-1 truncate">{{ store.name }}</span>
        <Money :amount="store.before" :currency="store.currency" class="text-muted-foreground" />
        <ArrowRight class="size-3 shrink-0 self-center text-muted-foreground" aria-hidden="true" />
        <Money :amount="store.after" :currency="store.currency" />
      </li>
    </ul>
  </section>
</template>
