<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

/** Credit or spend a programme's points; a credit can name the card that earned it. */
const props = defineProps<{ store: Store; mode: 'credit' | 'spend' }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const queryClient = useQueryClient()

const amount = ref('')
const comment = ref('')
const source = ref('')
watch(open, () => {
  amount.value = ''
  comment.value = ''
})

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
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
    toast.success(t('loyalty.saved'))
    for (const queryKey of [['cashback'], ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('loyalty.failed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.mode === 'credit' ? t('loyalty.credit') : t('loyalty.spend')">
    <form id="points-form" class="grid gap-4" @submit.prevent="Number(amount) > 0 && save.mutate()">
      <div class="grid gap-1.5">
        <Label for="points-amount">{{ t('loyalty.amount') }}</Label>
        <AmountInput id="points-amount" v-model="amount">
          <template #suffix>
            <span class="px-2 text-xs text-muted-foreground">{{ props.store.loyalty?.points_symbol }}</span>
          </template>
        </AmountInput>
      </div>
      <div class="grid gap-1.5">
        <Label for="points-comment">{{ t('loyalty.comment') }}</Label>
        <Input id="points-comment" v-model="comment" />
      </div>
      <div v-if="props.mode === 'credit'" class="grid gap-1.5">
        <Label for="points-source">{{ t('loyalty.source') }}</Label>
        <Select id="points-source" v-model="source">
          <option value="">{{ t('loyalty.noSource') }}</option>
          <option v-for="node in cards" :key="node.store.id" :value="node.store.id">{{ node.store.name }}</option>
        </Select>
      </div>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="points-form" :disabled="!(Number(amount) > 0) || save.isPending.value">
        {{ props.mode === 'credit' ? t('loyalty.credit') : t('loyalty.spend') }}
      </Button>
    </template>
  </Dialog>
</template>
