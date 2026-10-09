<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { StoreParentSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Textarea } from '@/shared/ui/textarea'
import { call, parseError } from '@/shared/api/ipc'

import type { StoreEditDialogProps } from './store-edit.types'

const props = defineProps<StoreEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const name = ref('')
const note = ref('')
const color = ref('')
const parentId = ref('')

watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = props.store.name
  note.value = props.store.note
  color.value = props.store.color
  parentId.value = props.store.parent_id ?? ''
})

const save = useMutation({
  mutationFn: () =>
    call<Store>('update_store', {
      input: {
        id: props.store.id,
        name: name.value.trim(),
        color: color.value,
        note: note.value.trim(),
        parent_id: parentId.value || null,
      },
    }),
  onSuccess: () => {
    toast.success(t('stores.saved'))
    // Moving a wallet changes the parents' rolled-up totals too.
    for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('stores.saveFailed'), { description: parseError(err).message }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('stores.edit')" size="sm">
    <form id="store-edit-form" class="grid gap-4" @submit.prevent="save.mutate()">
      <div class="grid grid-cols-[1fr_auto] gap-4">
        <div class="grid gap-1.5">
          <Label for="store-edit-name">{{ t('stores.name') }}</Label>
          <Input id="store-edit-name" v-model="name" />
        </div>
        <div class="grid gap-1.5">
          <Label for="store-edit-color">{{ t('stores.color') }}</Label>
          <input
            id="store-edit-color"
            v-model="color"
            type="color"
            class="h-9 w-14 cursor-pointer rounded-md border border-input bg-transparent p-1"
          >
        </div>
      </div>
      <div class="grid gap-1.5">
        <Label for="store-edit-parent">{{ t('stores.parent') }}</Label>
        <StoreParentSelect
          id="store-edit-parent"
          v-model="parentId"
          :stores="stores.data.value ?? []"
          :exclude-id="props.store.id"
        />
        <p class="text-xs text-muted-foreground">{{ t('stores.parentHint') }}</p>
      </div>
      <div class="grid gap-1.5">
        <Label for="store-edit-note">{{ t('stores.note') }}</Label>
        <Textarea id="store-edit-note" v-model="note" rows="2" />
      </div>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="store-edit-form" type="submit" :disabled="!name.trim() || save.isPending.value">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
