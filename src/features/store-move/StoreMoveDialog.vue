<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { StoreMoveDialogProps } from './store-move.types'

const props = defineProps<StoreMoveDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

// The rules (depth, cycles, sibling names) live in the backend; the dialog
// only offers what it says is allowed.
const targets = useQuery({
  queryKey: computed(() => ['stores', 'move-targets', props.store.id]),
  queryFn: () => call<StoreMoveTargets>('store_move_targets', { id: props.store.id }),
  enabled: open,
})

/** New parent id; `''` makes it a top-level wallet. */
const parentId = ref('')
const hasTargets = computed(() => (targets.data.value?.parents.length ?? 0) > 0 || targets.data.value?.can_detach)

watch(open, (isOpen) => {
  if (isOpen) parentId.value = ''
})

// Pick the first allowed parent once the list arrives, unless detaching is the only way.
watch(
  () => targets.data.value,
  (data) => {
    if (!data || parentId.value) return
    if (!data.can_detach && data.parents.length) parentId.value = data.parents[0].store.id
  },
)

const move = useMutation({
  mutationFn: () => call<Store>('move_store', { input: { id: props.store.id, parent_id: parentId.value || null } }),
  onSuccess: (store) => {
    toast.success(t('stores.moved'), { description: store.name })
    // Totals of the old and the new parent both change.
    for (const queryKey of [['stores'], ['store'], ['store-history'], ['store-pnl'], ['summary']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    open.value = false
  },
  onError: (err) => toast.error(t('stores.moveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('stores.moveTitle', { name: props.store.name })" size="sm">
    <p v-if="targets.isPending.value" class="text-sm text-muted-foreground">{{ t('store.loading') }}</p>
    <p v-else-if="!hasTargets" class="text-sm text-muted-foreground">{{ t('stores.moveNowhere') }}</p>
    <form v-else id="store-move-form" class="grid gap-1.5" @submit.prevent="move.mutate()">
      <Label for="store-move-target">{{ t('stores.moveTo') }}</Label>
      <StoreNodeSelect
        id="store-move-target"
        v-model="parentId"
        :nodes="targets.data.value?.parents ?? []"
        :empty-label="targets.data.value?.can_detach ? t('stores.makeStandalone') : undefined"
      />
      <p class="text-xs text-muted-foreground">{{ t('stores.moveHint') }}</p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="store-move-form" type="submit" :disabled="!hasTargets || move.isPending.value">
        {{ t('stores.move') }}
      </Button>
    </template>
  </Dialog>
</template>
