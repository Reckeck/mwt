<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { formatMoney } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import type { GoalDeleteProps } from '../goal-delete.types'
import { useGoalDelete } from '../model/use-goal-delete'

defineOptions({ inheritAttrs: false })

const props = defineProps<GoalDeleteProps>()
const emit = defineEmits<{ deleted: [] }>()

const { t } = useI18n()
const open = ref(false)
const remove = useGoalDelete()

/** Frame 5: what happens to the money first, then to the automation and the files. */
const money = computed(() => {
  const { goal, saved } = props.progress
  return props.wallet && Number(saved) > 0
    ? t('goalDelete.money', { wallet: props.wallet, saved: formatMoney(saved, { currency: goal.currency, decimals: 0 }) })
    : t('goalDelete.moneyFree')
})
const rest = computed(() =>
  [props.filler ? t('goalDelete.filler', { name: props.filler }) : '', t('goalDelete.files')].filter(Boolean).join(' '),
)

function confirm() {
  remove.mutate(props.progress.goal.id, {
    onSuccess: () => {
      open.value = false
      emit('deleted')
    },
  })
}
</script>

<template>
  <Button v-bind="$attrs" variant="outline" data-goal-delete @click="open = true">
    <Trash2 class="size-4" />
    {{ props.label ?? t('actions.delete') }}
  </Button>
  <Dialog v-model:open="open" size="sm" :title="t('goalDelete.title', { name: props.progress.goal.name })">
    <div class="grid gap-3 px-4 pb-2 text-sm" data-goal-delete-text>
      <p>{{ money }}</p>
      <p class="text-muted-foreground">{{ rest }}</p>
    </div>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" :disabled="remove.isPending.value" data-goal-delete-confirm @click="confirm">
        {{ t('goalDelete.confirm') }}
      </Button>
    </template>
  </Dialog>
</template>
