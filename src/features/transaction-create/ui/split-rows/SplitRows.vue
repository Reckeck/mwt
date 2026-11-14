<script setup lang="ts">
import { useI18n } from 'vue-i18n'

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
  <div class="flex min-w-0 flex-col gap-2" data-tour="tx-split">
    <SplitRowItem
      v-for="(row, index) in rows"
      :key="row.id"
      v-model="rows[index]"
      :kind="props.kind"
      :via="props.via"
      :index="index"
      @remove="remove(index)"
    />
    <Button
      type="button"
      variant="link"
      class="touch-min h-auto self-start px-0 py-0"
      data-tx-add-account
      @click="add"
    >{{ t('tx.split.addAccount') }}</Button>
  </div>
</template>
