<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { FreedomRateForm } from '@/features/freedom-rate-edit'
import { Card } from '@/shared/ui/card'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { StatCard } from '@/shared/ui/stat-card'

import { percent } from '../lib/reason-text'

/** How much of the essential spending passive income covers: in fact and at the expected return. */
const props = defineProps<{ passive: PassiveIncome; rate: string | null; keyRate: string | null }>()
const { t } = useI18n()

const fill = (pct: string | null) => Math.min(100, Number(pct ?? 0))
</script>

<template>
  <Card data-tour="health-freedom">
    <template #header>
      <p class="font-medium">{{ t('health.freedom.title') }}</p>
    </template>
    <div class="grid gap-4">
      <div class="grid gap-1">
        <p class="text-sm">
          {{
            props.passive.freedom_fact_pct === null
              ? t('health.freedom.noBase')
              : t('health.freedom.fact', { pct: percent(props.passive.freedom_fact_pct) })
          }}
        </p>
        <ProgressBar :value="fill(props.passive.freedom_fact_pct)" :label="t('health.freedom.title')" />
      </div>
      <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <StatCard :label="t('health.freedom.total')">
          <Money :amount="props.passive.total_12m" :currency="props.passive.currency" />
        </StatCard>
        <StatCard :label="t('health.freedom.invested')">
          <Money :amount="props.passive.invested" :currency="props.passive.currency" />
        </StatCard>
      </div>
      <p v-if="props.passive.freedom_potential_pct !== null" class="text-sm text-muted-foreground">
        {{
          t('health.freedom.potential', {
            pct: percent(props.passive.freedom_potential_pct),
            rate: percent(props.passive.expected_pct),
          })
        }}
      </p>
      <FreedomRateForm :rate="props.rate" :key-rate="props.keyRate" />
    </div>
  </Card>
</template>
