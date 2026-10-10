<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { UtilityDialog } from '@/features/utility-edit'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'

import UtilityCard from './UtilityCard.vue'

const { t } = useI18n()

const items = useQuery({ queryKey: ['utilities'], queryFn: () => call<UtilityItem[]>('list_utility_items') })
const createOpen = ref(false)
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('utilities.intro') }}</p>
      <Button @click="createOpen = true">
        <Plus class="size-4" />
        {{ t('utilities.new') }}
      </Button>
    </div>
    <div
      v-if="items.isSuccess.value && !items.data.value?.length"
      class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground"
    >
      {{ t('utilities.empty') }}
    </div>
    <div class="grid gap-4 @lg:grid-cols-2">
      <UtilityCard v-for="item in items.data.value ?? []" :key="item.id" :item="item" />
    </div>
    <UtilityDialog v-model:open="createOpen" />
  </div>
</template>
