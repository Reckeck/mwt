<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { useTxContext } from '@/shared/lib/tx-context'

import type { LossSummaryProps } from './loss-summary.types'

const props = withDefaults(defineProps<LossSummaryProps>(), { report: null, storeIds: () => [], hideEmpty: false, title: '' })

const { t } = useI18n()

/** Rows a group shows before «Показать ещё». */
const SHOWN = 8
const expanded = ref(new Set<string>())
function toggle(key: string) {
  const next = new Set(expanded.value)
  if (!next.delete(key)) next.add(key)
  expanded.value = next
}
const context = useTxContext()

/** Without a report handed in: the last twelve months in the budget currency. */
const own = useQuery({
  queryKey: computed(() => ['report', 'losses', 'summary', props.storeIds]),
  placeholderData: keepPreviousData,
  queryFn: () =>
    call<AnalyticsReport>('get_report', {
      kind: 'losses',
      filter: {
        scope: { store_ids: props.storeIds, include_children: true },
        from: null,
        to: null,
        year: null,
        currency: null,
        real_terms: false,
      },
    }),
  enabled: computed(() => props.report === null),
})
const data = computed<LossReport | null>(() => {
  if (props.report) return props.report
  const value = own.data.value
  return value && 'losses' in value ? value.losses : null
})

const groups = computed(() => {
  const value = data.value
  if (!value) return []
  return [
    { key: 'byKind', rows: value.by_kind, label: (key: string) => t(`tx.kind.${key}`), filter: 'kind' },
    { key: 'byStore', rows: value.by_store, label: (key: string) => context.storeName(key), filter: 'store' },
    { key: 'byContact', rows: value.by_contact, label: (key: string) => context.contactName(key), filter: 'contact' },
  ].filter((group) => group.rows.length && !(group.key === 'byStore' && props.storeIds.length))
})
</script>

<template>
  <div v-if="data && !(props.hideEmpty && !Number(data.total.total))" class="grid gap-4">
    <h3 v-if="props.title" class="heading-card">{{ props.title }}</h3>
    <div class="grid gap-4 @xl:grid-cols-3">
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('losses.total') }}</p>
        <p class="text-xl font-semibold tabular-nums"><Money :amount="data.total.total" :currency="data?.meta.currency" /></p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('losses.fee') }}</p>
        <p class="text-xl font-semibold tabular-nums"><Money :amount="data.total.fee" :currency="data?.meta.currency" /></p>
      </Card>
      <Card>
        <p class="text-xs text-muted-foreground">{{ t('losses.slippage') }}</p>
        <p class="text-xl font-semibold tabular-nums"><Money :amount="data.total.slippage" :currency="data?.meta.currency" /></p>
      </Card>
    </div>
    <p v-if="data.incomplete" class="text-xs text-warning">
      {{ t('losses.incompleteCount', { count: data.incomplete }) }}
    </p>
    <div class="grid gap-4 @wide:grid-cols-3">
      <Card v-for="group in groups" :key="group.key">
        <template #header>
          <h3 class="heading-card">{{ t(`losses.${group.key}`) }}</h3>
        </template>
        <ul class="grid gap-1 text-sm">
          <li
            v-for="row in expanded.has(group.key) ? group.rows : group.rows.slice(0, SHOWN)"
            :key="row.key"
            class="flex justify-between gap-2"
          >
            <!-- The transactions behind the row, in the journal with the same filter. -->
            <RouterLink
              :to="{ name: 'transactions', query: { [group.filter]: row.key } }"
              class="min-w-0 truncate hover:underline"
            >
              {{ group.label(row.key) }}
            </RouterLink>
            <span class="shrink-0 tabular-nums"><Money :amount="row.total" :currency="data?.meta.currency" /></span>
          </li>
        </ul>
        <Button
          v-if="group.rows.length > SHOWN"
          variant="link"
          size="sm"
          class="mt-1 h-auto w-fit px-0"
          @click="toggle(group.key)"
        >
          {{ expanded.has(group.key) ? t('ui.showLess') : t('ui.showMore', { count: group.rows.length - SHOWN }) }}
        </Button>
      </Card>
    </div>
    <p v-if="!groups.length" class="py-6 text-center text-sm text-muted-foreground">{{ t('losses.empty') }}</p>
  </div>
</template>
