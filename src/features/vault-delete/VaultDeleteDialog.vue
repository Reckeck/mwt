<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Dialog } from '@/shared/ui/dialog'
import { formatDayMonthShort } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'

import { useVaultDelete } from './model/use-vault-delete'

const props = defineProps<{
  /** The history row to erase; `null` keeps the dialog closed. */
  recent: RecentVault | null
}>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const isPhone = useIsPhone()
const { erase, saveCopy } = useVaultDelete()

const name = computed(() => props.recent?.display_name || props.recent?.path || '')
const title = computed(() => t(isPhone.value ? 'vaultDelete.titleNamed' : 'vaultDelete.title', { name: name.value }))
/** What the history knows of the file: it lives in the app, last opened on this day. */
const detail = computed(() => {
  const opened = Number(props.recent?.last_opened)
  if (!Number.isFinite(opened) || opened <= 0) return t('vaultDelete.inApp')
  return `${t('vaultDelete.inApp')} · ${formatDayMonthShort(new Date(opened * 1000).toISOString())}`
})

function copy() {
  if (props.recent) saveCopy.mutate(props.recent.path)
}

function remove() {
  if (!props.recent) return
  erase.mutate(props.recent.path, { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <Dialog overlay="vault-delete" v-model:open="open" :title="title" size="confirm">
    <div class="grid gap-3">
      <p class="text-sm">{{ t('vaultDelete.body') }}</p>
      <Button
        v-if="isPhone"
        variant="link"
        class="justify-start px-0"
        :loading="saveCopy.isPending.value"
        data-vault-copy
        @click="copy"
      >
        {{ t('vaultDelete.copyFirst') }}
      </Button>
      <Card>
        <p class="truncate text-sm font-medium">{{ name }}</p>
        <p class="truncate text-xs text-muted-foreground">{{ detail }}</p>
      </Card>
    </div>

    <template #footer>
      <Button v-if="!isPhone" variant="outline" class="mr-auto" :loading="saveCopy.isPending.value" data-vault-copy @click="copy">
        {{ t('vaultDelete.saveCopy') }}
      </Button>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" :loading="erase.isPending.value" data-vault-erase @click="remove">
        <Trash2 class="size-4" />
        {{ t('actions.delete') }}
      </Button>
    </template>
  </Dialog>
</template>
