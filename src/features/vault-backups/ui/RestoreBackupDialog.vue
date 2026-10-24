<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { TriangleAlert } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { formatDateTime } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

const props = defineProps<{ backups: VaultBackup[]; pending: boolean }>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ restore: [name: string] }>()
const { t } = useI18n()

/** Nothing is pre-picked on purpose: replacing the vault is a choice, not a default. */
const picked = ref('')
watch(open, () => {
  picked.value = ''
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('vaultBackups.restoreTitle')" :description="t('vaultBackups.restoreHint')">
    <div class="grid gap-4">
      <RadioGroup v-model="picked" class="grid gap-2" data-vault-backup-list>
        <label
          v-for="backup in props.backups"
          :key="backup.name"
          :for="`vault-backup-${backup.name}`"
          :class="
            cn(
              'flex min-h-10 cursor-pointer items-center gap-3 rounded-lg border border-border p-3 transition-colors hover:bg-accent',
              'has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
              picked === backup.name && 'border-primary bg-primary/5',
            )
          "
        >
          <RadioGroupItem :id="`vault-backup-${backup.name}`" :value="backup.name" />
          <span class="grid min-w-0 gap-0.5">
            <span class="truncate text-sm font-medium">{{ backup.name }}</span>
            <span class="text-xs text-muted-foreground">{{ formatDateTime(backup.modified_at) }}</span>
          </span>
        </label>
      </RadioGroup>
      <p class="flex items-start gap-2 text-xs text-muted-foreground">
        <TriangleAlert class="mt-0.5 size-4 shrink-0 text-warning" aria-hidden="true" />
        {{ t('vaultBackups.restoreWarn') }}
      </p>
    </div>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" :disabled="!picked" :loading="props.pending" @click="emit('restore', picked)">
        {{ t('vaultBackups.restoreSubmit') }}
      </Button>
    </template>
  </Dialog>
</template>
