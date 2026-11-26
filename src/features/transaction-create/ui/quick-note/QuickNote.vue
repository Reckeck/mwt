<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pencil } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

/**
 * The note of an operation: the «Заметка» chip of board 533927 that opens a one-line field on a line
 * of its own; a filled note opens it already.
 */
const props = defineProps<{ id?: string }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const open = ref(model.value !== '')
</script>

<template>
  <div v-if="open" class="basis-full">
    <Input :id="props.id" v-model="model" :placeholder="t('quickAdd.notePlaceholder')" :aria-label="t('quickAdd.note')" />
  </div>
  <div v-else>
    <Button type="button" variant="outline" class="h-9 gap-1.5 rounded-full px-3 text-sm font-normal [&_svg]:size-4" data-quick-note @click="open = true">
      <Pencil aria-hidden="true" />{{ t('quickAdd.note') }}
    </Button>
  </div>
</template>
