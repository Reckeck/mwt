<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import type { SplitRow } from '../../transaction-form.types'
import SplitRowItem from './SplitRowItem.vue'
import type { SplitRowsProps } from './split-rows.types'

/**
 * More accounts the money lands on, after the first one the form shows
 * itself: an exchange whose counterparty pays out to several of your
 * accounts, a top-up spread over several. Saved as one chain.
 */
const props = defineProps<SplitRowsProps>()
const rows = defineModel<SplitRow[]>({ required: true })

const { t } = useI18n()

function add() {
  rows.value = [
    ...rows.value,
    { id: crypto.randomUUID(), to: null, receive: '' },
  ]
}

function remove(index: number) {
  rows.value = rows.value.filter((_, at) => at !== index)
}
</script>

<template>
  <div class="grid gap-2" data-tour="tx-split">
    <SplitRowItem
      v-for="(row, index) in rows"
      :key="row.id"
      v-model="rows[index]"
      :kind="props.kind"
      :mode="props.mode"
      :index="index"
      @remove="remove(index)"
    />
    <div class="flex flex-wrap items-center gap-2">
      <Button type="button" variant="outline" size="sm" @click="add">
        <Plus class="size-4" />
        {{ t('tx.split.addAccount') }}
      </Button>
      <span v-if="rows.length" class="text-xs text-muted-foreground">{{ t('tx.split.linkedHint') }}</span>
    </div>
  </div>
</template>
