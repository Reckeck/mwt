<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { Badge } from '@/shared/ui/badge'
import { SectionState } from '@/shared/ui/section-state'

/** «Что выгоднее?»: the same plan with 2 points more rate against 20% more contribution. */
const props = defineProps<{ input: InvestInput }>()

const { t } = useI18n()

const compare = useQuery({
  queryKey: computed(() => ['calc-invest-compare', props.input]),
  queryFn: () => call<InvestCompare>('calc_invest_compare', { input: props.input }),
  placeholderData: keepPreviousData,
})

const rows = computed(() => compare.data.value?.rows ?? [])
const recommended = (kind: CompareKind) => kind === 'more_contribution' && compare.data.value?.contribution_wins
</script>

<template>
  <section class="grid gap-2" data-tour="calc-compare">
    <h3 class="text-sm font-medium">{{ t('invest.compare.title') }}</h3>
    <p class="text-sm text-muted-foreground">{{ t('invest.compare.hint') }}</p>
    <SectionState :loading="compare.isPending.value" :error="compare.error.value" skeleton="list" :rows="3" compact>
      <ul class="grid gap-2">
        <li
          v-for="row in rows"
          :key="row.kind"
          class="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 rounded-md border px-3 py-2"
        >
          <div class="grid gap-0.5">
            <span class="flex items-center gap-2 text-sm font-medium">
              {{ t(`invest.compare.kind.${row.kind}`) }}
              <Badge v-if="recommended(row.kind)" variant="secondary">{{ t('invest.compare.recommended') }}</Badge>
            </span>
            <span class="text-xs text-muted-foreground">{{ t(`invest.compare.note.${row.kind}`) }}</span>
          </div>
          <div class="grid justify-items-end gap-0.5 text-sm">
            <Money :amount="row.final_nominal" :currency="props.input.currency" :decimals="0" />
            <Money v-if="row.kind !== 'base'" :amount="row.gain" :currency="props.input.currency" :decimals="0" colored />
          </div>
        </li>
      </ul>
    </SectionState>
  </section>
</template>
