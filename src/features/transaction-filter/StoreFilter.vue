<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'

/** Selected store id; an empty string means every store. */
const storeId = defineModel<string>({ required: true })

const { t } = useI18n()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})
</script>

<template>
  <Select v-model="storeId" class="h-9 w-56">
    <option value="">{{ t('tx.allStores') }}</option>
    <option v-for="store in stores.data.value ?? []" :key="store.id" :value="store.id">{{ store.name }}</option>
  </Select>
</template>
