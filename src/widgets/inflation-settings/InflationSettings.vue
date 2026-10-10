<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settings = useSettingsStore()
const queryClient = useQueryClient()

/** Editable copy: `[code, percent]`, written on every change. */
const rows = ref<Array<[string, string]>>([])
watch(
  () => settings.app?.inflation,
  (table) => {
    rows.value = Object.entries(table ?? {}).map(([code, rate]) => [code, String(rate)])
  },
  { immediate: true, deep: true },
)

const free = computed(() => settings.allCurrencies.filter((code) => !rows.value.some(([known]) => known === code)))
const adding = ref('')

async function persist() {
  const table: Record<string, string> = {}
  for (const [code, rate] of rows.value) {
    const value = Number(rate.replace(',', '.'))
    if (code && Number.isFinite(value)) table[code] = String(value)
  }
  try {
    await settings.updateApp({ inflation: table })
    toast.success(t('settings.saved'))
    void queryClient.invalidateQueries({ queryKey: ['report'] })
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) })
  }
}

function add() {
  if (!adding.value) return
  rows.value.push([adding.value, '0'])
  adding.value = ''
  void persist()
}

function remove(index: number) {
  rows.value.splice(index, 1)
  void persist()
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('inflation.title') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('inflation.hint') }}</p>
    </template>
    <div class="grid gap-2">
      <div v-for="(row, index) in rows" :key="row[0]" class="flex items-center gap-2">
        <span class="w-16 text-sm font-medium uppercase">{{ row[0] }}</span>
        <Input
          v-model="row[1]"
          inputmode="decimal"
          class="w-24"
          :aria-label="t('inflation.rateFor', { currency: row[0].toUpperCase() })"
          @change="persist"
        />
        <span class="text-sm text-muted-foreground">{{ t('inflation.perYear') }}</span>
        <Button
          variant="ghost"
          size="icon"
          class="ml-auto size-8 text-muted-foreground"
          :aria-label="t('inflation.remove', { currency: row[0].toUpperCase() })"
          @click="remove(index)"
        >
          <Trash2 class="size-4" />
        </Button>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="adding" class="w-32" :aria-label="t('inflation.add')">
          <option value="" disabled>{{ t('inflation.pick') }}</option>
          <option v-for="code in free" :key="code" :value="code">{{ code.toUpperCase() }}</option>
        </Select>
        <Button variant="outline" size="sm" :disabled="!adding" @click="add">
          <Plus class="size-4" />
          {{ t('inflation.add') }}
        </Button>
      </div>
    </div>
  </Card>
</template>
