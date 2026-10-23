<script setup lang="ts">
import { toRef } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowRight, ReceiptText } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { useSpending } from './model/use-spending'
import { SpendingAnswer, SpendingCategories, SpendingMonths } from './ui/spending'

/** «Траты» (Money 1 · 1): the answer, the twelve months, the categories. */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const { rows, balance, months, monthKeys, currency, loading, error, refetch } = useSpending(toRef(props, 'filter'))

/** A month's bar opens that month, with the rest of the filter kept. */
function pick(month: string) {
  void router.replace({ query: { ...route.query, from: month, to: month } })
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState :loading="loading" :error="error" skeleton="report" @retry="refetch">
      <template v-if="balance.data.value">
        <SpendingAnswer
          :from="props.filter.from ?? ''"
          :to="props.filter.to ?? ''"
          :balance="balance.data.value"
          :average="months.data.value?.average_spent ?? '0'"
          :rows="rows"
        />
        <SpendingMonths
          v-if="months.data.value"
          :balance="months.data.value"
          :from="props.filter.from ?? ''"
          :to="props.filter.to ?? ''"
          @pick="pick"
        />
        <SpendingCategories v-if="rows.length" :rows="rows" :currency="currency" :filter="props.filter" :months="monthKeys" />
        <EmptyState v-else compact :icon="ReceiptText" :title="t('money.categories.empty')">
          <Button variant="outline" size="sm" @click="router.push({ name: 'transactions' })">{{ t('money.categories.toTransactions') }}</Button>
        </EmptyState>
        <div class="flex justify-end">
          <Button variant="secondary" class="max-md:h-11 max-md:w-full" @click="router.push({ name: 'money-where', query: route.query })">
            {{ t('money.next.where') }} <ArrowRight />
          </Button>
        </div>
      </template>
    </SectionState>
  </div>
</template>
