<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { HandCoins } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import { useTemplateActions } from '../model/use-template-actions'
import TemplateDialog from './TemplateDialog.vue'

/** Ready-made scenarios: a card each, a short form, then an ordinary workflow. */
const { t } = useI18n()
const { templates } = useTemplateActions()
const picked = ref<WorkflowTemplateInfo | null>(null)
const dialogOpen = ref(false)

function pick(template: WorkflowTemplateInfo) {
  picked.value = template
  dialogOpen.value = true
}
</script>

<template>
  <section class="grid gap-3" data-tour="workflow-template-gallery">
    <div>
      <h2 class="text-base font-semibold">{{ t('workflowTemplates.title') }}</h2>
      <p class="text-sm text-muted-foreground">{{ t('workflowTemplates.hint') }}</p>
    </div>
    <SectionState :loading="templates.isPending.value" :error="templates.error.value" skeleton="list" :rows="3" compact>
      <div class="grid gap-3 @2xl:grid-cols-2 @6xl:grid-cols-3">
        <Card v-for="template in templates.data.value ?? []" :key="template.id">
          <div class="flex h-full flex-col gap-2">
          <div class="flex items-start justify-between gap-2">
            <h3 class="text-sm font-medium">{{ t(`workflowTemplates.${template.id}.title`) }}</h3>
            <Badge v-if="template.moves_money" variant="outline" class="shrink-0">
              <HandCoins class="size-3" />
              {{ t('workflowTemplates.movesMoney') }}
            </Badge>
          </div>
          <p class="flex-1 text-xs text-muted-foreground">{{ t(`workflowTemplates.${template.id}.desc`) }}</p>
          <Button variant="outline" size="sm" class="self-start" @click="pick(template)">
            {{ t('workflowTemplates.use') }}
          </Button>
          </div>
        </Card>
      </div>
    </SectionState>
    <TemplateDialog v-model:open="dialogOpen" :template="picked" />
  </section>
</template>
