<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { StoreParentSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'

import { DEFAULT_STORE_COLOR, STORE_KINDS } from './store-create.constants'
import type { StoreCreateDialogProps } from './store-create.types'

const props = withDefaults(defineProps<StoreCreateDialogProps>(), { parentId: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const name = ref('')
const color = ref(DEFAULT_STORE_COLOR)
const kind = ref<StoreKind>('cash')
const parentId = ref('')

const parent = computed(() => stores.data.value?.find((store) => store.id === parentId.value) ?? null)

// Every opening starts clean. A sub-wallet starts as its parent's kind and
// colour — a Bybit sub-account is an exchange too — and stays editable.
watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = ''
  parentId.value = props.parentId ?? ''
  kind.value = parent.value?.kind ?? 'cash'
  color.value = parent.value?.color ?? DEFAULT_STORE_COLOR
})

const createStore = useMutation({
  mutationFn: () =>
    call<Store>('create_store', {
      input: {
        name: name.value.trim(),
        kind: kind.value,
        color: color.value,
        note: '',
        parent_id: parentId.value || null,
      },
    }),
  onSuccess: (store) => {
    toast.success(t('stores.created'), { description: store.name })
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
    void queryClient.invalidateQueries({ queryKey: ['store'] })
    void queryClient.invalidateQueries({ queryKey: ['summary'] })
    open.value = false
    emit('created', store)
  },
  onError: (err) => toast.error(t('stores.createFailed'), { description: parseError(err).message }),
})
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="parent ? t('stores.newSubWallet', { parent: parent.name }) : t('wallet.newStore')"
    size="sm"
  >
    <form id="store-create-form" class="grid gap-4" @submit.prevent="createStore.mutate()">
      <div class="grid gap-1.5">
        <Label for="store-name">{{ t('stores.name') }}</Label>
        <Input id="store-name" v-model="name" :placeholder="t('stores.namePlaceholder')" />
      </div>
      <div class="grid gap-1.5">
        <Label for="store-parent">{{ t('stores.parent') }}</Label>
        <StoreParentSelect id="store-parent" v-model="parentId" :stores="stores.data.value ?? []" />
        <p class="text-xs text-muted-foreground">{{ t('stores.parentHint') }}</p>
      </div>
      <div class="grid grid-cols-[1fr_auto] gap-4">
        <div class="grid gap-1.5">
          <Label for="store-kind">{{ t('stores.kind') }}</Label>
          <Select id="store-kind" v-model="kind">
            <option v-for="entry in STORE_KINDS" :key="entry" :value="entry">{{ t(`stores.kind.${entry}`) }}</option>
          </Select>
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
