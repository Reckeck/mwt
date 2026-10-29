<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { UtilityBillDialog } from '@/features/utility-bill'
import { call } from '@/shared/api/ipc'
import { localDate } from '@/shared/lib/planning'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'

import UtilityBillCard from './UtilityBillCard.vue'
import UtilityBillLines from './UtilityBillLines.vue'
import { UtilityItemSheet } from '../utility-item'

/**
 * A home's first tab (Wallets-2 · 10): this month's bill on top, its items
 * below. Paying and typing the readings open the bill, an item opens its own panel; a new item comes
 * from `+ Item` beside the tabs (below the list on a phone).
 */
const props = defineProps<{ store: Store }>()
const emit = defineEmits<{ newItem: [] }>()
const { t } = useI18n()
const phone = useIsPhone()

const period = localDate(new Date()).slice(0, 7)
const bill = useQuery({
  // no placeholder: another home's bill must not show under this one
  queryKey: computed(() => ['utility-bill', props.store.id, period]),
  queryFn: () => call<UtilityBill>('get_utility_bill', { propertyId: props.store.id, period }),
})
const lines = computed(() => bill.data.value?.lines ?? [])
const billOpen = ref(false)
/** The item whose side panel is open (Wallets-2 · 11–12); kept while it slides out. */
const picked = ref<string | null>(null)
const itemOpen = ref(false)
const pickedLine = computed(() => lines.value.find((line) => line.item.id === picked.value) ?? null)
function openItem(line: UtilityBillLine) {
  picked.value = line.item.id
  itemOpen.value = true
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <SectionState
      :loading="bill.isPending.value"
      :error="bill.error.value"
      :empty="bill.isSuccess.value && !lines.length"
      :empty-title="t('utilities.empty')"
      :empty-description="t('utilities.intro')"
      @retry="bill.refetch()"
    >
      <template #empty>
        <Button @click="emit('newItem')"><Plus class="size-4" />{{ t('utilityTab.newItem') }}</Button>
      </template>
      <div v-if="bill.data.value" class="flex flex-col gap-4">
        <UtilityBillCard :store="props.store" :bill="bill.data.value" :phone="phone" @pay="billOpen = true" @readings="billOpen = true" />
        <UtilityBillLines
          :bill="bill.data.value" :phone="phone" :disabled="props.store.archived"
          :active="itemOpen ? picked : null"
          @open="openItem"
        />
        <Button v-if="phone" variant="outline" class="h-11" data-utility-new @click="emit('newItem')">
          <Plus class="size-4" />{{ t('utilityTab.newItem') }}
        </Button>
      </div>
    </SectionState>
    <UtilityBillDialog v-model:open="billOpen" :property-id="props.store.id" />
    <UtilityItemSheet v-model:open="itemOpen" :line="pickedLine" :period="period" :phone="phone" :disabled="props.store.archived" />
  </div>
</template>
