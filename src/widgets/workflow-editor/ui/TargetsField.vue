<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Trash2 } from 'lucide-vue-next'

import { AccountRefPicker } from '@/features/transaction-create'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

/** `Distribute`: where the money goes and how much of it, row by row. */
interface Target {
  account: AccountRef | null
  share: string
}

const props = defineProps<{ value: unknown; percent: boolean }>()
const emit = defineEmits<{ update: [value: Target[]] }>()

const { t } = useI18n()
const rows = computed<Target[]>(() => (Array.isArray(props.value) ? (props.value as Target[]) : []))
const total = computed(() => rows.value.reduce((sum, row) => sum + (Number(String(row.share).replace(',', '.')) || 0), 0))

function change(index: number, patch: Partial<Target>) {
  emit(
    'update',
    rows.value.map((row, at) => (at === index ? { ...row, ...patch } : row)),
  )
}
</script>

<template>
  <div class="grid gap-2">
    <div v-for="(row, index) in rows" :key="index" class="grid grid-cols-[minmax(0,1fr)_5rem_2rem] items-center gap-2">
      <AccountRefPicker
        :model-value="row.account"
        :contacts="false"
        :raw="false"
        @update:model-value="(account) => change(index, { account })"
      />
      <Input
        :model-value="row.share"
        inputmode="decimal"
        :aria-label="props.percent ? t('workflow.targets.share') : t('workflow.targets.amount')"
        @update:model-value="(share) => change(index, { share: String(share) })"
      />
      <Button
        variant="ghost"
        size="icon"
        :aria-label="t('workflow.targets.remove')"
        @click="emit('update', rows.filter((_, at) => at !== index))"
      >
        <Trash2 class="size-4" />
      </Button>
    </div>
    <Button variant="outline" size="sm" class="w-fit" @click="emit('update', [...rows, { account: null, share: '' }])">
      <Plus class="size-4" />
      {{ t('workflow.targets.add') }}
    </Button>
    <p v-if="props.percent && rows.length" class="text-xs text-muted-foreground">
      {{ t('workflow.targets.total', { total }) }}
    </p>
  </div>
</template>
