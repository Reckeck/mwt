<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { PiggyBank } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { EmergencyFundDialog, useFundAdd } from '@/features/emergency-fund-edit'
import { StoreCreateDialog } from '@/features/store-create'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { EntityLink } from '@/shared/ui/entity-link'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { formatQuantity } from '@/shared/lib/format'

import { percent } from '../lib/reason-text'

/** The emergency fund: months covered against the target, its accounts and what they earn. */
const props = defineProps<{ fund: EmergencyStatus; names: Record<string, string> }>()
const { t } = useI18n()

const editOpen = ref(false)
const createOpen = ref(false)
const add = useFundAdd()

const target = computed(() => props.fund.fund.target_months)
const covered = computed(() => Number(props.fund.months_covered ?? 0))
const fill = computed(() => (target.value ? (covered.value / target.value) * 100 : 0))
const tone = computed(() => (covered.value < 1 ? 'destructive' : covered.value < 3 ? 'warning' : 'primary'))
/** The 1/3/6 levels inside the target, as marks under the bar. */
const levels = computed(() =>
  [1, 3, 6]
    .filter((months) => months <= target.value)
    .map((months) => ({
      months,
      left: `${(months / target.value) * 100}%`,
      amount: props.fund.e_mand === null ? null : String(Number(props.fund.e_mand) * months),
    })),
)
const set = computed(() => props.fund.stores.length > 0)
</script>

<template>
  <Card data-tour="health-fund">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('health.fund.title') }}</p>
        <Button variant="outline" size="sm" @click="editOpen = true">{{ t('health.fund.edit') }}</Button>
      </div>
    </template>

    <EmptyState
      v-if="!set"
      compact
      :icon="PiggyBank"
      :title="t('health.fund.emptyTitle')"
      :description="t(props.fund.candidates.length ? 'health.fund.emptyPick' : 'health.fund.emptyCreate')"
    >
      <ul v-if="props.fund.candidates.length" class="grid w-full gap-2">
        <li v-for="id in props.fund.candidates" :key="id" class="flex items-center justify-between gap-2">
          <EntityLink :target="{ kind: 'store', id }" :label="props.names[id] ?? ''" />
          <Button size="sm" variant="outline" :loading="add.isPending.value && add.variables.value === id" @click="add.mutate(id)">
            {{ t('health.fund.make') }}
          </Button>
        </li>
      </ul>
      <Button v-else size="sm" @click="createOpen = true">{{ t('health.fund.create') }}</Button>
    </EmptyState>

    <div v-else class="grid gap-4">
      <div class="grid gap-1">
        <p class="text-2xl font-semibold tabular-nums">
          {{
            props.fund.months_covered === null
              ? t('health.fund.noSpending')
              : t('health.fund.months', { months: formatQuantity(props.fund.months_covered, 1), target })
          }}
        </p>
        <p class="text-sm text-muted-foreground">
          <Money :amount="props.fund.total" :currency="props.fund.currency" />
          <template v-if="props.fund.e_mand !== null">
            · {{ t('health.fund.base') }} <Money :amount="props.fund.e_mand" :currency="props.fund.currency" />
          </template>
        </p>
      </div>
      <div class="grid gap-1">
        <ProgressBar :value="fill" :tone="tone" :label="t('health.fund.title')" />
        <div class="relative h-8 text-xs text-muted-foreground">
          <span
            v-for="level in levels"
            :key="level.months"
            class="absolute top-0 -translate-x-full text-right whitespace-nowrap first:translate-x-0 first:text-left"
            :style="{ left: level.left }"
          >
            {{ t('health.fund.level', { months: level.months }) }}
            <br v-if="level.amount" />
            <Money v-if="level.amount" :amount="level.amount" :currency="props.fund.currency" />
          </span>
        </div>
      </div>
      <p v-if="props.fund.surplus" class="text-sm text-success">
        {{ t('health.fund.surplus') }} <Money :amount="props.fund.surplus" :currency="props.fund.currency" />
      </p>
      <ul class="grid gap-2 text-sm">
        <li v-for="row in props.fund.stores" :key="row.store_id" class="grid gap-0.5">
          <div class="flex items-center justify-between gap-2">
            <EntityLink :target="{ kind: 'store', id: row.store_id }" :label="props.names[row.store_id] ?? ''" />
            <Money :amount="row.value" :currency="props.fund.currency" />
          </div>
          <p v-if="row.rate" class="text-xs text-muted-foreground">
            {{ t('health.fund.rate', { rate: percent(row.rate) }) }}
            <template v-if="Number(row.interest_12m) > 0">
              · {{ t('health.fund.interest') }} <Money :amount="row.interest_12m" :currency="props.fund.currency" />
            </template>
          </p>
          <p v-if="row.below_inflation" class="text-xs text-warning">
            {{ t('health.fund.belowInflation', { inflation: percent(row.inflation) }) }}
          </p>
          <p v-if="row.goal_ids.length" class="text-xs text-muted-foreground">
            {{ t('health.fund.alsoGoal') }}
            <EntityLink
              v-for="goal in row.goal_ids"
              :key="goal"
              :target="{ kind: 'goal', id: goal }"
              :label="props.names[goal] ?? ''"
              class="ml-1"
            />
          </p>
        </li>
      </ul>
    </div>

    <EmergencyFundDialog v-model:open="editOpen" :fund="props.fund.fund" />
    <StoreCreateDialog v-model:open="createOpen" />
  </Card>
</template>
