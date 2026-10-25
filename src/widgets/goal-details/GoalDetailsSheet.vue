<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pencil, Plus } from 'lucide-vue-next'

import { MoneyPhrase } from '@/entities/currency'
import { AttachmentsBlock } from '@/features/attachment-edit'
import { GoalDelete } from '@/features/goal-delete'
import { GoalDialog } from '@/features/goal-edit'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { Button } from '@/shared/ui/button'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { EntityBack } from '@/shared/ui/entity-link'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { formatPct } from '@/shared/lib/format'
import { composeSums, partsText } from '@/shared/lib/money-parts'

import type { GoalDetailsSheetProps } from './goal-details.types'
import { useGoalDetails } from './model/use-goal-details'
import GoalEditFiller from './ui/GoalEditFiller.vue'
import GoalFacts from './ui/GoalFacts.vue'

/**
 * The goal's card (New-Goal 3): saved of the target, where and by when, what
 * fills it, the forecast and the deposits; its files and links; «Пополнить»
 * writes a transfer into its wallet.
 */
const props = withDefaults(defineProps<GoalDetailsSheetProps>(), { groups: () => [] })
const open = defineModel<boolean>('open', { required: true })

defineSlots<{ wallet?(props: { storeId: string; label: string }): unknown }>()

const { t } = useI18n()
const details = useGoalDetails(
  () => props.goalId,
  () => props.groups,
  () => open.value,
)
const entry = details.entry

const summary = computed(() => {
  if (!entry.value) return null
  const { goal, saved, pct } = entry.value
  const left = Math.max(0, Number(goal.target_amount) - Number(saved))
  const { mark, parts } = composeSums()
  const sum = (amount: string | number, bare = false) => mark({ amount, currency: goal.currency, decimals: 0, bare })
  const leftParts = parts(t('goalCard.left', { pct: formatPct(pct, { decimals: 0 }), left: sum(left) }))
  return {
    saved: parts(t('goalCard.savedOf', { saved: sum(saved, true), target: sum(goal.target_amount) })),
    pct: Math.min(100, Math.max(0, Number(pct))),
    left: leftParts,
    leftText: partsText(leftParts),
  }
})

const cardBody = ref<HTMLElement | null>(null)
const owner = computed<EntityRef>(() => ({ kind: 'goal', id: props.goalId ?? '' }))
const ownerLabel = computed(() => (entry.value ? t('goalCard.dropOwner', { name: entry.value.goal.name }) : ''))

const editOpen = ref(false)
function closeAll() {
  editOpen.value = false
  open.value = false
}
const topUpOpen = ref(false)
const counterpart = computed<AccountRef | null>(() => (details.store.value ? { store: details.store.value } : null))
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent side="right" class="flex w-full flex-col gap-0 p-0 sm:max-w-md" data-goal-card
      :data-entity-card="props.goalId ? `goal:${props.goalId}` : undefined"
    >
      <EntityBack class="mx-2 mt-2" />
      <div v-if="entry && summary" ref="cardBody" class="relative flex min-h-0 flex-1 flex-col">
        <div class="grid flex-1 content-start gap-5 overflow-y-auto pt-2 pb-6">
          <SheetHeader class="gap-0.5 pe-12">
            <SheetTitle class="text-lg">{{ entry.goal.name }}</SheetTitle>
            <SheetDescription class="text-xs">{{ t('goalCard.subtitle') }}</SheetDescription>
          </SheetHeader>

          <div class="grid gap-5 px-4">
            <div class="grid gap-2" data-goal-progress>
              <p class="text-base font-medium tabular-nums"><MoneyPhrase :parts="summary.saved" /></p>
              <ProgressBar :value="summary.pct" :label="summary.leftText" />
              <p class="text-sm text-muted-foreground tabular-nums"><MoneyPhrase :parts="summary.left" /></p>
            </div>

            <GoalFacts :entry="entry" :store="details.store.value" :store-name="details.storeName.value" :filler="details.filler.value">
              <template #wallet="slotProps"><slot name="wallet" v-bind="slotProps" /></template>
            </GoalFacts>

            <AttachmentsBlock :owner="owner" :owner-label="ownerLabel" :drop-target="cardBody" :active="open && !editOpen" />
          </div>
        </div>

        <footer class="flex items-center gap-2 border-t px-4 pt-3 pb-[max(0.75rem,var(--safe-bottom,0px))]" data-goal-card-actions>
          <GoalDelete
            :progress="entry"
            :wallet="details.storeName.value"
            :filler="details.filler.value?.label ?? null"
            class="max-sm:min-w-0 max-sm:flex-1"
            @deleted="open = false"
          />
          <Button variant="outline" class="max-sm:min-w-0 max-sm:flex-1 sm:ms-auto" :disabled="!details.source.value" data-goal-top-up @click="topUpOpen = true">
            <Plus class="size-4 shrink-0" />
            {{ t('goalCard.topUp') }}
          </Button>
          <Button class="max-sm:min-w-0 max-sm:flex-1" data-goal-edit @click="editOpen = true">
            <Pencil class="size-4 shrink-0" />
            {{ t('actions.edit') }}
          </Button>
        </footer>

        <GoalDialog v-model:open="editOpen" :goal="entry.goal" :saved="entry.saved">
          <template #extras>
            <GoalEditFiller :filler="details.filler.value" />
            <AttachmentsBlock :owner="owner" :owner-label="ownerLabel" :active="editOpen" />
          </template>
          <template #actions>
            <GoalDelete
              :progress="entry"
              :wallet="details.storeName.value"
              :filler="details.filler.value?.label ?? null"
              :label="t('goal.delete')"
              class="text-destructive hover:text-destructive max-md:h-11"
              @deleted="closeAll"
            />
          </template>
        </GoalDialog>
        <TransactionCreateDialog
          v-if="topUpOpen && details.source.value"
          v-model:open="topUpOpen"
          :store-id="details.source.value"
          :counterpart="counterpart"
        />
      </div>
    </SheetContent>
  </Sheet>
</template>
