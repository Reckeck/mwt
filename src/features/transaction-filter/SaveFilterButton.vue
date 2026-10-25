<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pin } from 'lucide-vue-next'

import { usePins } from '@/shared/api/pins'
import { required, useFormValidation } from '@/shared/lib/form'
import { activeFilterCount, txFilterToQuery } from '@/shared/lib/tx-filter'
import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

const props = defineProps<{ filter: TxFilter }>()

const { t } = useI18n()
const pins = usePins()

const open = ref(false)
const name = ref('')
const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: open })

watch(open, (isOpen) => {
  if (isOpen) name.value = ''
})

/** Only what is set goes into the address the pin opens. */
const query = computed(() => {
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(txFilterToQuery(props.filter))) if (value !== undefined) params.set(key, value)
  return params.toString()
})

function save() {
  pins.saveFilter(name.value.trim(), query.value)
  open.value = false
}
</script>

<template>
  <Button variant="outline" :disabled="!activeFilterCount(props.filter)" data-tour="tx-save-filter" @click="open = true">
    <Pin class="size-4" />
    {{ t('pins.saveFilter') }}
  </Button>

  <FormDialog overlay="tx-save-filter"
    v-model:open="open"
    :title="t('pins.saveFilterTitle')"
    :description="t('pins.saveFilterHint')"
    form-id="save-filter-form"
    :submit-label="t('pins.saveFilter')"
    :pending="pins.pending.value"
    @submit="form.submit(save, $event)"
  >
    <FormField for="save-filter-name" :label="t('pins.filterName')" required :error="form.error('name')">
      <Input id="save-filter-name" v-model="name" :placeholder="t('pins.filterNamePlaceholder')" />
    </FormField>
  </FormDialog>
</template>
