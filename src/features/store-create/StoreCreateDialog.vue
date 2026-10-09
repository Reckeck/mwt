<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'

import { DEFAULT_STORE_COLOR, STORE_KINDS } from './store-create.constants'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const color = ref(DEFAULT_STORE_COLOR)
const kind = ref<StoreKind>('cash')

const createStore = useMutation({
  mutationFn: () => call<Store>('create_store', { input: { name: name.value, kind: kind.value, color: color.value, note: '' } }),
  onSuccess: (store) => {
    toast.success(t('stores.created'), { description: store.name })
    name.value = ''
    void queryClient.invalidateQueries({ queryKey: ['stores'] })
  },
  onError: (err) => toast.error(t('stores.createFailed'), { description: parseError(err).message }),
})

async function submit() {
  const created = await createStore.mutateAsync().catch(() => null)
  if (created) open.value = false
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('wallet.newStore')" size="sm">
    <template #close>
      <Button variant="ghost" size="icon" @click="open = false">
        <X class="size-4" />
        <span class="sr-only">{{ t('actions.close') }}</span>
      </Button>
    </template>

    <form class="grid gap-4" @submit.prevent="submit">
      <div class="grid gap-1.5">
        <Label for="sname">{{ t('stores.name') }}</Label>
        <Input id="sname" v-model="name" :placeholder="t('stores.name')" />
      </div>
      <div class="grid gap-1.5">
        <Label for="skind">{{ t('stores.kind') }}</Label>
        <select id="skind" v-model="kind" class="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm">
          <option v-for="entry in STORE_KINDS" :key="entry" :value="entry">{{ t(`stores.kind.${entry}`) }}</option>
        </select>
      </div>
      <div class="grid gap-1.5">
        <Label for="scolor">{{ t('stores.color') }}</Label>
        <input id="scolor" v-model="color" type="color" class="h-9 w-20 cursor-pointer rounded-md border border-input bg-transparent">
      </div>

      <div class="flex justify-end gap-2">
        <Button type="button" variant="outline" @click="open = false">
          {{ t('actions.cancel') }}
        </Button>
        <Button type="submit" :disabled="!name.trim() || createStore.isPending.value">
          <Plus class="size-4" />
          {{ t('stores.create') }}
        </Button>
      </div>
    </form>
  </Dialog>
</template>
