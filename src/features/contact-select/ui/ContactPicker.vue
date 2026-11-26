<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { useContacts } from '@/shared/api/queries'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { PickerRow, PickerSheet } from '@/shared/ui/picker-sheet'

/**
 * The counterparty of an operation: the saved contacts (not archived) in a bottom sheet with a
 * search field and a «no counterparty» choice. `''` means nobody. Choosing stays here (rule 14).
 */
const props = defineProps<{ id?: string; invalid?: boolean }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const open = ref(false)
const search = ref('')

const contacts = useContacts()
const options = computed(() => (contacts.data.value ?? []).filter((contact) => !contact.archived || contact.id === model.value))
const current = computed(() => options.value.find((contact) => contact.id === model.value))
const found = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return needle ? options.value.filter((contact) => contact.name.toLowerCase().includes(needle)) : options.value
})

function pick(id: string) {
  model.value = id
  open.value = false
  search.value = ''
}
</script>

<template>
  <PickerSheet v-model:open="open" v-model:search="search" overlay="contact-select" :title="t('quickAdd.contact')" :search-label="t('quickAdd.searchContact')">
    <template #trigger>
      <Button
        :id="props.id"
        type="button"
        variant="outline"
        :aria-invalid="props.invalid || undefined"
        :class="cn('w-full justify-between gap-2 px-3 font-normal', !current && 'text-muted-foreground', props.invalid && 'border-destructive')"
      >
        <span class="truncate">{{ current?.name ?? t('quickAdd.pickContact') }}</span>
        <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
      </Button>
    </template>
    <PickerRow :title="t('quickAdd.noContact')" :selected="!model" @click="pick('')" />
    <PickerRow
      v-for="contact in found"
      :key="contact.id"
      :title="contact.name"
      :selected="contact.id === model"
      @click="pick(contact.id)"
    />
  </PickerSheet>
</template>
