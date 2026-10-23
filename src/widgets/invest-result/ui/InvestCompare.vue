<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'

/**
 * «Что выгоднее»: the same plan with 2 points more rate against 20 % more
 * contribution; «Подставить» moves the form to that plan.
 */
const props = defineProps<{ input: InvestInput }>()
const emit = defineEmits<{ apply: [kind: CompareKind] }>()

const { t } = useI18n()

const compare = useQuery({
  queryKey: computed(() => ['calc-invest-compare', props.input]),
  queryFn: () => call<InvestCompare>('calc_invest_compare', { input: props.input }),
  placeholderData: keepPreviousData,
})

const rows = computed(() => compare.data.value?.rows ?? [])
const best = computed(() => {
  const changed = rows.value.filter((row) => row.kind !== 'base')
  return changed.reduce<CompareRow | null>((top, row) => (!top || Number(row.gain) > Number(top.gain) ? row : top), null)?.kind
})
</script>

<template>
  <section class="grid gap-2" data-tour="calc-compare">
    <h3 class="text-sm font-medium" :title="t('invest.compare.hint')">{{ t('invest.compare.title') }}</h3>
    <SectionState :loading="compare.isPending.value" :error="compare.error.value" skeleton="list" :rows="3" compact>
      <ul class="grid divide-y rounded-md border">
        <li
          v-for="row in rows"
          :key="row.kind"
          class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 px-3 py-2 @md:grid-cols-[minmax(0,1fr)_auto_auto_6rem]"
          data-calc-compare-row
        >
          <span class="flex min-w-0 flex-wrap items-center gap-x-2 text-sm" :title="t(`invest.compare.note.${row.kind}`)">
            {{ t(`invest.compare.kind.${row.kind}`) }}
            <Badge v-if="row.kind === best" variant="secondary">{{ t('invest.compare.best') }}</Badge>
          </span>
          <Money
            v-if="row.kind !== 'base'"
            :amount="row.gain"
            :currency="props.input.currency"
            :decimals="0"
            colored
            signed
            class="text-right text-sm @max-md:row-start-2 @max-md:text-left"
          />
          <span v-else class="@max-md:hidden" />
          <Money :amount="row.final_nominal" :currency="props.input.currency" :decimals="0" class="text-right text-sm font-medium" />
          <Button
            v-if="row.kind !== 'base'"
            variant="link"
            size="sm"
            class="h-auto justify-self-end px-0 @max-md:col-start-2"
            @click="emit('apply', row.kind)"
          >
            {{ t('invest.compare.apply') }}
          </Button>
        </li>
      </ul>
    </SectionState>
  </section>
</template>
