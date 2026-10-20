<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { FormField } from '@/shared/ui/form-field'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useStoreTree } from '@/shared/api/queries'

/** Credit or spend a programme's points; a credit can name the card that earned it. */
const props = defineProps<{ store: Store; mode: 'credit' | 'spend' }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const queryClient = useQueryClient()

const amount = ref('')
const comment = ref('')
const source = ref('')

const form = useFormValidation(() => ({ amount: positiveAmount(amount.value) }), { resetOn: open })
watch(open, () => {
  amount.value = ''
  comment.value = ''
})

const tree = useStoreTree()
const cards = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived && node.store.kind === 'bank'),
)

const save = useMutation({
  mutationFn: () =>
    call<Transaction>('adjust_points', {
      input: {
        store_id: props.store.id,
        delta: props.mode === 'credit' ? amount.value : `-${amount.value}`,
        comment: comment.value.trim(),
        source_store_id: props.mode === 'credit' && source.value ? source.value : null,
        timestamp: null,
      } satisfies AdjustPointsInput,
    }),
  onSuccess: () => {
    for (const queryKey of [['cashback'], ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'loyalty.failed', success: 'loyalty.saved' },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.mode === 'credit' ? t('loyalty.credit') : t('loyalty.spend')">
    <form id="points-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField for="points-amount" :label="t('loyalty.amount')" required :error="form.error('amount')">
        <AmountInput id="points-amount" v-model="amount">
          <template #suffix>
            <span class="px-2 text-xs text-muted-foreground">{{ props.store.loyalty?.points_symbol }}</span>
          </template>
        </AmountInput>
      </FormField>
      <div class="grid gap-1.5">
        <Label for="points-comment">{{ t('loyalty.comment') }}</Label>
        <Input id="points-comment" v-model="comment" />
      </div>
      <div v-if="props.mode === 'credit'" class="grid gap-1.5">
        <Label for="points-source">{{ t('loyalty.source') }}</Label>
        <StoreNodeSelect id="points-source" v-model="source" :nodes="cards" flat :empty-label="t('loyalty.noSource')" />
      </div>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="points-form" :disabled="save.isPending.value">
        {{ props.mode === 'credit' ? t('loyalty.credit') : t('loyalty.spend') }}
      </Button>
    </template>
  </Dialog>
</template>
