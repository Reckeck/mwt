<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Archive } from 'lucide-vue-next'

import { Money } from '@/entities/currency'

/**
 * The capital the move takes out of the totals (spec E11): the sum, the wallets by name with their own
 * amounts and the promise that nothing is deleted. Nothing at all when no hand-kept wallet is retired.
 * No placeholder while the plan is read: the step cannot know whether there will be a block at all, and the
 * block is the last child of step 1, so one that appears moves nothing already on screen.
 */
const props = defineProps<{ plan: CryptoTransitionPlan | undefined }>()
const { t } = useI18n()

const wallets = computed(() => props.plan?.wallets ?? [])
</script>

<template>
  <section
    v-if="props.plan && wallets.length"
    class="grid gap-3 rounded-lg border border-border bg-muted/40 p-3 text-sm"
    data-raise-capital
  >
    <div class="flex items-start gap-3">
      <Archive class="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <div class="grid min-w-0 gap-1">
        <p class="font-medium">{{ t('securityRaise.capital.title', wallets.length) }}</p>
        <p class="text-lg font-semibold tabular-nums" data-raise-capital-total>
          <Money :amount="props.plan.leaving_base" :currency="props.plan.base_currency" />
        </p>
      </div>
    </div>
    <ul class="grid gap-1">
      <li v-for="wallet in wallets" :key="wallet.store_id" class="flex items-baseline justify-between gap-3">
        <span class="min-w-0 truncate">{{ wallet.name }}</span>
        <Money class="shrink-0" :amount="wallet.leaving_base" :currency="props.plan.base_currency" />
      </li>
    </ul>
    <p class="text-muted-foreground">{{ t('securityRaise.capital.kept') }}</p>
  </section>
</template>
