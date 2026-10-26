<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import { useWorkflowActions } from '../../model/use-workflow-actions'

/**
 * «Запустить «Подписки»?» (Full-New-Fixes 7): the operation as it will be written —
 * sum in its own currency, wallet, category (only if there is one), the run's date.
 * The run does it with the values it saved, so they are shown, not edited.
 */
const props = defineProps<{ entry: JournalEntry | null; storeName: (id: string) => string }>()
const open = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const { answerWait } = useWorkflowActions()
const categories = useCategories()

const money = computed(() => props.entry?.money ?? null)
const from = computed(() => (money.value?.from_store ? props.storeName(money.value.from_store) : ''))
const into = computed(() => (money.value?.to_store ? props.storeName(money.value.to_store) : ''))
const category = computed(() => (money.value?.category ? categoryKeyLabel(money.value.category, categories.data.value, t) : ''))

function answer(yes: boolean) {
  const id = props.entry?.wait_id
  if (!id) return
  answerWait.mutate({ id, yes }, { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <Dialog overlay="automation-confirm" v-model:open="open" size="sm" :title="t('automations.confirm.runTitle', { name: props.entry?.name ?? '' })">
    <div class="grid gap-3 px-4 pb-2 text-sm" data-automation-confirm>
      <p class="text-muted-foreground">{{ t('automations.confirm.written') }}</p>
      <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2">
        <dt class="text-muted-foreground">{{ t('automations.confirm.amount') }}</dt>
        <dd class="text-right font-semibold tabular-nums" data-automation-confirm-amount>
          <Money v-if="money?.amount" :amount="money.amount" :currency="money.currency ?? undefined" :decimals="2" :at="props.entry?.at" />
          <template v-else>—</template>
        </dd>
        <template v-if="from">
          <dt class="text-muted-foreground">{{ t('automations.confirm.fromWallet') }}</dt>
          <dd class="min-w-0 truncate text-right">{{ from }}</dd>
        </template>
        <template v-if="into">
          <dt class="text-muted-foreground">{{ t('automations.confirm.toWallet') }}</dt>
          <dd class="min-w-0 truncate text-right">{{ into }}</dd>
        </template>
        <template v-if="category">
          <dt class="text-muted-foreground">{{ t('automations.confirm.category') }}</dt>
          <dd class="min-w-0 truncate text-right">{{ category }}</dd>
        </template>
        <dt class="text-muted-foreground">{{ t('automations.confirm.date') }}</dt>
        <dd class="text-right">{{ formatDate(props.entry?.at) }}</dd>
      </dl>
    </div>
    <template #footer>
      <Button variant="outline" class="max-md:h-11" :disabled="answerWait.isPending.value" data-automation-skip @click="answer(false)">
        {{ t('actions.cancel') }}
      </Button>
      <Button class="max-md:h-11" :disabled="answerWait.isPending.value" data-automation-yes @click="answer(true)">
        {{ t('automations.confirm.run') }}
      </Button>
    </template>
  </Dialog>
</template>
