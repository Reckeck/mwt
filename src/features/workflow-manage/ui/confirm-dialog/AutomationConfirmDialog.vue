<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatDayMonthShort, numberFormat } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import { useWorkflowActions } from '../../model/use-workflow-actions'

/**
 * «Подтвердить: N» (Auto-1 · 5): what a run waiting for «да» is about to do —
 * the sum, the share of the income it was taken from, where from and where to.
 * The run does it with the values it saved, so they are shown, not edited.
 */
const props = defineProps<{ entry: JournalEntry | null; storeName: (id: string) => string }>()
const open = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const { answerWait } = useWorkflowActions()
const categories = useCategories()

const sourceId = computed(() => props.entry?.source_id ?? null)
const card = useQuery({
  queryKey: computed(() => [...WORKFLOW_KEYS.list, 'card', sourceId.value]),
  queryFn: () => call<AutomationCard>('get_automation_card', { id: sourceId.value }),
  enabled: computed(() => open.value && sourceId.value !== null),
  // no placeholder: another automation's percent must not show under this run
})

const money = computed(() => props.entry?.money ?? null)
/** «10 % от 1 575 $»: the income the share was taken from. */
const share = computed(() => {
  const percent = card.data.value?.id === sourceId.value ? card.data.value?.actions[0]?.percent : null
  const sum = Number(money.value?.amount)
  if (!percent || !sum || !money.value?.currency) return ''
  const income = (sum * 100) / Number(percent)
  return { percent: numberFormat({ maximumFractionDigits: 0 }).format(Number(percent)), income, currency: money.value.currency, decimals: Number.isInteger(income) ? 0 : 2 }
})
const lead = computed(() => {
  const date = formatDayMonthShort(props.entry?.at)
  const trigger = card.data.value?.id === sourceId.value ? card.data.value?.trigger : null
  if (trigger?.kind === 'transaction' && trigger.category) {
    return t('automations.confirm.leadAfter', { date, category: categoryKeyLabel(trigger.category, categories.data.value, t) })
  }
  return t('automations.confirm.lead', { date })
})
const from = computed(() => (money.value?.from_store ? props.storeName(money.value.from_store) : '—'))
const to = computed(() => (money.value?.to_store ? props.storeName(money.value.to_store) : '—'))

function answer(yes: boolean) {
  const id = props.entry?.wait_id
  if (!id) return
  answerWait.mutate({ id, yes }, { onSuccess: () => (open.value = false) })
}

const FIELD = 'flex items-center rounded-md border border-input bg-muted/40 px-3'
</script>

<template>
  <Dialog overlay="automation-confirm"
    v-model:open="open"
    size="form"
    :title="t('automations.confirm.title', { name: props.entry?.name ?? '' })"
  >
    <div class="flex flex-col gap-4" data-automation-confirm>
      <p class="text-sm text-muted-foreground">{{ lead }}</p>
      <div class="flex flex-col gap-1.5">
        <span class="text-sm font-medium">{{ t('automations.confirm.amount') }}</span>
        <div :class="[FIELD, 'h-13 gap-2 text-2xl font-semibold tabular-nums']" data-automation-confirm-amount>
          <span class="min-w-0 flex-1 truncate">
            <Money v-if="money?.amount" :amount="money.amount" :currency="money.currency ?? undefined" :decimals="2" :at="props.entry?.at" />
            <template v-else>—</template>
          </span>
        </div>
        <i18n-t v-if="share" keypath="automations.confirm.share" tag="span" class="text-xs text-muted-foreground">
          <template #percent>{{ share.percent }}</template>
          <template #income><Money :amount="share.income" :currency="share.currency" :decimals="share.decimals" :at="props.entry?.at" /></template>
        </i18n-t>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div class="flex min-w-0 flex-col gap-1.5">
          <span class="text-sm font-medium">{{ t('automations.confirm.from') }}</span>
          <div :class="[FIELD, 'h-10 text-sm']"><span class="truncate">{{ from }}</span></div>
        </div>
        <div class="flex min-w-0 flex-col gap-1.5">
          <span class="text-sm font-medium">{{ t('automations.confirm.to') }}</span>
          <div :class="[FIELD, 'h-10 text-sm']"><span class="truncate">{{ to }}</span></div>
        </div>
      </div>
    </div>
    <template #footer>
      <Button variant="outline" :disabled="answerWait.isPending.value" data-automation-skip @click="answer(false)">
        {{ t('automations.confirm.skip') }}
      </Button>
      <Button :disabled="answerWait.isPending.value" data-automation-yes @click="answer(true)">
        {{ t('automations.confirm.yes') }}
      </Button>
    </template>
  </Dialog>
</template>
