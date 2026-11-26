<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

/** The note of an operation: a chip that opens a one-line field; a filled note opens it already. */
const props = defineProps<{ id?: string }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const open = ref(model.value !== '')
</script>

<template>
  <Input v-if="open" :id="props.id" v-model="model" :placeholder="t('quickAdd.notePlaceholder')" :aria-label="t('quickAdd.note')" />
  <div v-else>
    <Button type="button" size="sm" variant="outline" class="rounded-full" @click="open = true">{{ t('quickAdd.addNote') }}</Button>
  </div>
</template>
