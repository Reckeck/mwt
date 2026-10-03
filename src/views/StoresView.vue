<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Plus, Trash2, X } from 'lucide-vue-next'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Dialog from '@/components/ui/dialog.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { call } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import { getStoreIcon } from '@/lib/store-icons'

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const createDialogOpen = ref(false)
const name = ref('')
const color = ref('#0ea5e9')
const kind = ref<StoreKind>('cash')

const kinds: StoreKind[] = ['exchange', 'hot_wallet', 'cold_wallet', 'bank', 'cash', 'paper']

function invalidate() {
  return queryClient.invalidateQueries({ queryKey: ['stores'] })
}

const createStore = useMutation({
  mutationFn: () => call<Store>('create_store', { input: { name: name.value, kind: kind.value, color: color.value, note: '' } }),
  onSuccess: () => {
    name.value = ''
    void invalidate()
  },
})

async function submitCreate() {
  await createStore.mutateAsync()
  createDialogOpen.value = false
}

const toggleArchive = useMutation({
  mutationFn: (s: Store) => call('archive_store', { id: s.id, archived: !s.archived }),
  onSuccess: () => void invalidate(),
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_store', { id }),
  onSuccess: () => void invalidate(),
})
</script>

<template>
  <div class="grid gap-6 @container">
    <div class="flex items-center justify-between gap-4">
      <p class="text-sm text-muted-foreground">{{ t('stores.title') }}</p>
      <Button @click="createDialogOpen = true">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </div>

    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <Card v-for="s in stores.data.value ?? []" :key="s.id" :class="cn(s.archived && 'opacity-60')">
        <template #header>
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <span
                class="flex size-6 shrink-0 items-center justify-center rounded-md border"
                :style="{
                  backgroundColor: s.color + '1A',
                  color: s.color,
                  borderColor: s.color + '33',
                }"
              >
                <component :is="getStoreIcon(s.kind)" class="size-3.5" />
              </span>
              <p class="font-medium">{{ s.name }}</p>
            </div>
            <div class="flex items-center gap-1">
              <Badge variant="secondary">{{ t(`stores.kind.${s.kind}`) }}</Badge>
            </div>
          </div>
        </template>
        <div class="flex items-center justify-between gap-2">
          <p class="text-sm text-muted-foreground">{{ s.note || '—' }}</p>
          <div class="flex gap-1">
            <Button variant="ghost" size="icon" :title="s.archived ? t('stores.unarchive') : t('stores.archive')" @click="toggleArchive.mutate(s)">
              <ArchiveRestore v-if="s.archived" class="size-4" />
              <Archive v-else class="size-4" />
            </Button>
            <Button variant="ghost" size="icon" :title="t('stores.delete')" @click="remove.mutate(s.id)">
              <Trash2 class="size-4 text-destructive" />
            </Button>
          </div>
        </div>
      </Card>

      <Card v-if="!stores.data.value?.length">
        <p class="text-sm text-muted-foreground">{{ t('stores.empty') }}</p>
      </Card>
    </div>

    <Dialog v-model:open="createDialogOpen" :title="t('wallet.newStore')" size="sm">
      <template #close>
        <Button variant="ghost" size="icon" @click="createDialogOpen = false">
          <X class="size-4" />
          <span class="sr-only">{{ t('actions.close') }}</span>
        </Button>
      </template>

      <form class="grid gap-4" @submit.prevent="submitCreate">
        <div class="grid gap-1.5">
          <Label for="sname">{{ t('stores.name') }}</Label>
          <Input id="sname" v-model="name" :placeholder="t('stores.name')" />
        </div>
        <div class="grid gap-1.5">
          <Label for="skind">{{ t('stores.kind') }}</Label>
          <select
            id="skind"
            v-model="kind"
            class="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm"
          >
            <option v-for="k in kinds" :key="k" :value="k">{{ t(`stores.kind.${k}`) }}</option>
          </select>
        </div>
        <div class="grid gap-1.5">
          <Label for="scolor">{{ t('stores.color') }}</Label>
          <input
            id="scolor"
            v-model="color"
            type="color"
            class="h-9 w-20 cursor-pointer rounded-md border border-input bg-transparent"
          >
        </div>

        <div class="flex justify-end gap-2">
          <Button type="button" variant="outline" @click="createDialogOpen = false">
            {{ t('actions.cancel') }}
          </Button>
          <Button type="submit" :disabled="!name.trim() || createStore.isPending.value">
            <Plus class="size-4" />
            {{ t('stores.create') }}
          </Button>
        </div>
      </form>
    </Dialog>
  </div>
</template>