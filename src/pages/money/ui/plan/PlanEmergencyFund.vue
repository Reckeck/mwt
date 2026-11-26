<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { EmergencyFundDialog } from '@/features/emergency-fund-edit'
import { Money } from '@/entities/currency'
import { useEmergencyStatus } from '@/shared/api/queries'
import { formatQuantity } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { SectionState } from '@/shared/ui/section-state'

/** The emergency fund: how many months of essential spending the rainy-day accounts cover against the target. */
const { t } = useI18n()
const status = useEmergencyStatus()
const data = computed(() => status.data.value ?? null)
const set = computed(() => (data.value?.fund.scope.store_ids.length ?? 0) > 0)
const covered = computed(() => Number(data.value?.months_covered ?? 0))
const target = computed(() => data.value?.fund.target_months ?? 0)
const percent = computed(() => (target.value ? (covered.value / target.value) * 100 : 0))

const open = ref(false)
</script>

<template>
  <section class="grid content-start gap-3 rounded-lg border border-border bg-card p-4" data-tour="plan-fund">
    <header class="flex items-center justify-between gap-2">
      <h2 class="heading-section">{{ t('emergency.switch') }}</h2>
      <Button v-if="data" variant="outline" size="sm" data-fund-edit @click="open = true">
        {{ set ? t('emergency.change') : t('emergency.set') }}
      </Button>
    </header>
    <SectionState :loading="status.isPending.value" :error="status.error.value" skeleton="figure" compact @retry="status.refetch()">
      <div v-if="data" class="grid gap-2">
        <template v-if="set">
          <p class="text-sm tabular-nums">
            {{ t('emergency.coverage', { months: formatQuantity(covered, 1), target }) }}
          </p>
          <ProgressBar :value="percent" :tone="percent >= 100 ? 'primary' : 'warning'" :label="t('emergency.switch')" />
          <p class="text-xs text-muted-foreground tabular-nums">
            <Money :amount="data.total" :currency="data.currency" :decimals="0" />
            <template v-if="data.target_amount">
              / <Money :amount="data.target_amount" :currency="data.currency" :decimals="0" />
            </template>
          </p>
        </template>
        <p v-else class="text-sm text-muted-foreground">{{ t('emergency.none') }}</p>
        <p class="text-xs text-muted-foreground">{{ t('emergency.switchHint') }}</p>
      </div>
    </SectionState>
    <EmergencyFundDialog v-if="data" v-model:open="open" :fund="data.fund" :status="data" />
  </section>
</template>
