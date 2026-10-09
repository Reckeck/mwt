<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { DEFAULT_STORE_COLOR, STORE_KINDS } from './store-create.constants'
import type { StoreCreateDialogProps } from './store-create.types'

const props = withDefaults(defineProps<StoreCreateDialogProps>(), { parent: null, parentPath: () => [] })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const color = ref(DEFAULT_STORE_COLOR)
const kind = ref<StoreKind>('cash')
const note = ref('')

const title = computed(() => {
  if (!props.parent) return t('wallet.newStore')
  const path = props.parentPath.length ? props.parentPath : [props.parent.name]
  return t('stores.newSubAccount', { parent: path.join(' › ') })
})

/** The kind still matches the parent's: say where it came from. */
const kindInherited = computed(() => props.parent !== null && kind.value === props.parent.kind)

// Every opening starts clean. A sub-account starts as its parent's kind and
// colour — a Bybit sub-account is an exchange too — and stays editable.
watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = ''
  note.value = ''
  kind.value = props.parent?.kind ?? 'cash'
  color.value = props.parent?.color ?? DEFAULT_STORE_COLOR
})

const createStore = useMutation({
  mutationFn: () =>
    call<Store>('create_store', {
      input: {
        name: name.value.trim(),
        kind: kind.value,
        color: color.value,
        note: note.value.trim(),
        parent_id: props.parent?.id ?? null,
      },
    }),
  onSuccess: (store) => {
    toast.success(props.parent ? t('stores.subAccountCreated') : t('stores.created'), { description: store.name })
    for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('created', store)
  },
  onError: (err) => toast.error(t('stores.createFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="title" size="sm">
    <form id="store-create-form" class="grid gap-4" @submit.prevent="createStore.mutate()">
      <div class="grid gap-1.5">
        <Label for="store-name">{{ t('stores.name') }}</Label>
        <Input id="store-name" v-model="name" :placeholder="t('stores.namePlaceholder')" />
      </div>
      <div class="grid grid-cols-[1fr_auto] gap-4">
        <div class="grid gap-1.5">
          <Label for="store-kind">{{ t('stores.kind') }}</Label>
          <Select id="store-kind" v-model="kind">
            <option v-for="entry in STORE_KINDS" :key="entry" :value="entry">{{ t(`stores.kind.${entry}`) }}</option>
          </Select>
          <p v-if="kindInherited && props.parent" class="text-xs text-muted-foreground">
            {{ t('stores.kindInherited', { parent: props.parent.name }) }}
          </p>
        </div>
        <div class="grid gap-1.5">
          <Label for="store-color">{{ t('stores.color') }}</Label>
          <input
            id="store-color"
            v-model="color"
            type="color"
            class="h-9 w-14 cursor-pointer rounded-md border border-input bg-transparent p-1"
          >
        </div>
      </div>
      <div class="grid gap-1.5">
        <Label for="store-note">{{ t('stores.note') }}</Label>
        <Textarea id="store-note" v-model="note" rows="2" />
      </div>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="store-create-form" type="submit" :disabled="!name.trim() || createStore.isPending.value">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </template>
  </Dialog>
</template>
