<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Landmark } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Badge, type BadgeVariant } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { EntityLink } from '@/shared/ui/entity-link'
import { StatCard } from '@/shared/ui/stat-card'

import { percent } from '../lib/reason-text'

/** Debt load: DTI, monthly payments, what is owed and each loan with its class. */
const props = defineProps<{ debts: DebtLoad; names: Record<string, string> }>()
const { t } = useI18n()

const BADGE: Record<LoanClass, BadgeVariant> = {
  costly: 'destructive',
  normal: 'secondary',
  cheap: 'outline',
  unknown: 'outline',
}
/** Above this DTI the light turns red: the figure is drawn in the same colour. */
const DTI_HIGH = 50
const dtiTone = computed(() => (Number(props.debts.dti_pct ?? 0) > DTI_HIGH ? 'negative' : 'neutral'))
</script>

<template>
  <Card data-tour="health-debts">
    <template #header>
      <p class="font-medium">{{ t('health.debts.title') }}</p>
    </template>
    <EmptyState
      v-if="!props.debts.loans.length"
      compact
      :icon="Landmark"
      :title="t('health.debts.emptyTitle')"
      :description="t('health.debts.empty')"
    />
    <div v-else class="grid gap-4">
      <div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <StatCard
          :label="t('health.debts.dti')"
          :value="props.debts.dti_pct === null ? '—' : percent(props.debts.dti_pct)"
          :hint="t('health.debts.dtiHint', { months: props.debts.income_months })"
          :tone="dtiTone"
        />
        <StatCard :label="t('health.debts.payments')">
          <Money :amount="props.debts.payments" :currency="props.debts.currency" />
        </StatCard>
        <StatCard :label="t('health.debts.owed')">
          <Money :amount="props.debts.owed" :currency="props.debts.currency" />
        </StatCard>
      </div>
      <ul class="grid gap-2 text-sm">
        <li v-for="loan in props.debts.loans" :key="loan.store_id" class="grid gap-0.5">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <EntityLink :target="{ kind: 'store', id: loan.store_id }" :label="props.names[loan.store_id] ?? ''" />
            <Badge :variant="BADGE[loan.class]">{{ t(`health.loanClass.${loan.class}`) }}</Badge>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t(`liabilityKind.${loan.kind}`) }}
            <template v-if="loan.rate"> · {{ t('health.debts.rate', { rate: percent(loan.rate) }) }}</template>
            · <Money :amount="loan.owed" :currency="props.debts.currency" />
            <template v-if="Number(loan.payment) > 0">
              · {{ t('health.debts.payment') }} <Money :amount="loan.payment" :currency="props.debts.currency" />
              <template v-if="loan.payment_from_fact"> ({{ t('health.debts.fromFact') }})</template>
            </template>
          </p>
        </li>
      </ul>
    </div>
  </Card>
</template>
