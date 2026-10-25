<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ChevronLeft, Pencil, Play, Trash2 } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { SectionState } from '@/shared/ui/section-state'
import { Separator } from '@/shared/ui/separator'
import { EntityBack } from '@/shared/ui/entity-link'
import { Sheet, SheetContent, SheetDescription, SheetTitle } from '@/shared/ui/sheet'
import { Switch } from '@/shared/ui/switch'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

import { useAutomationDetails } from './model/use-automation-details'
import AutomationFacts from './ui/AutomationFacts.vue'
import AutomationRuns from './ui/AutomationRuns.vue'

/**
 * The card of a scheme or a repeat (Auto-1 · 3): a drawer on the right on a
 * computer, a screen with «‹» on a phone. When and on what it fires, what it
 * does, whether it asks, the next time and the last runs; switch it, run it
 * now, delete it, open it in the editor.
 */
const props = defineProps<{ automationId: string | null }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const router = useRouter()
const phone = useIsPhone()
const details = useAutomationDetails(
  () => props.automationId,
  () => open.value,
)
const card = details.current

const active = computed({
  get: () => card.value?.active ?? false,
  set: (value: boolean) => details.setActive(value),
})

function edit() {
  if (!card.value) return
  const id = card.value.id
  open.value = false
  void router.push({ name: 'workflow-edit', params: { id } })
}
const footerButton = computed(() => phone.value && 'h-11 min-w-0 flex-1 px-2')
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent overlay="entity-automation" side="right" :hide-close="phone" class="flex w-full flex-col gap-0 p-0 max-sm:overflow-y-auto sm:max-w-[460px]" data-automation-card
      :data-entity-card="props.automationId ? `automation:${props.automationId}` : undefined"
    >
      <header :class="cn('flex shrink-0 items-center gap-1', phone ? 'h-14 px-2' : 'px-6 pt-5 pb-3 pe-12')">
        <Button v-if="phone" variant="ghost" size="icon" class="touch-hit" :aria-label="t('actions.close')" @click="open = false">
          <ChevronLeft class="size-5" />
        </Button>
        <SheetTitle :class="cn('min-w-0 truncate font-semibold', phone ? 'text-xl' : 'text-lg')">
          {{ card?.name ?? t('automationCard.title') }}
        </SheetTitle>
        <SheetDescription class="sr-only">{{ t('automationCard.subtitle') }}</SheetDescription>
      </header>

      <EntityBack class="mx-4 mb-1" />
      <SectionState :loading="details.card.isPending.value" :error="details.card.error.value" :empty="false" @retry="details.card.refetch()">
        <div v-if="card" class="flex min-h-0 flex-1 flex-col gap-3.5 overflow-y-auto overscroll-contain px-6 pt-1 pb-4 max-sm:flex-none max-sm:overflow-visible max-sm:px-4">
          <div class="flex items-center gap-2">
            <Badge variant="secondary">{{ t(`automations.mine.kind.${card.kind}`) }}</Badge>
            <Switch v-model="active" class="ms-auto" :aria-label="t('automations.mine.toggle', { name: card.name })" data-automation-switch />
          </div>
          <AutomationFacts :card="card" :names="details.names" />
          <Separator />
          <AutomationRuns :runs="card.runs" :names="details.names" />
        </div>
      </SectionState>

      <footer
        v-if="card"
        :class="cn('flex shrink-0 items-center gap-2.5 border-t px-6 pt-3.5 pb-[max(0.875rem,var(--safe-bottom,0px))]', phone && 'flex-wrap border-t-0 px-4 pt-0')"
        data-automation-card-actions
      >
        <ConfirmButton
          :label="t('actions.delete')"
          :confirm-label="t('automationCard.deleteConfirm')"
          show-label
          :disabled="details.removing.value"
          :class="footerButton"
          data-automation-delete
          @confirm="details.remove(() => (open = false))"
        >
          <Trash2 class="size-4 shrink-0" />
        </ConfirmButton>
        <Button variant="outline" :class="footerButton" :disabled="details.running.value" data-automation-run-now @click="details.runNow()">
          <Play v-if="!phone" class="size-4 shrink-0" />
          {{ t('automationCard.runNow') }}
        </Button>
        <Button :class="cn(footerButton, !phone && 'ms-auto')" data-automation-edit @click="edit">
          <Pencil class="size-4 shrink-0" />
          {{ t('actions.edit') }}
        </Button>
      </footer>
    </SheetContent>
  </Sheet>
</template>
