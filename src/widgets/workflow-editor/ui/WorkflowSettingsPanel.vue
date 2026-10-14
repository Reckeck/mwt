<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import { Textarea } from '@/shared/ui/textarea'
import { Button } from '@/shared/ui/button'

/** With nothing selected: the workflow's own settings and its problems. */
const props = defineProps<{
  issues: GraphIssue[]
  base: string
  /** Flow wires that repeat a data wire: one button removes them. */
  redundant: number
}>()
const description = defineModel<string>('description', { required: true })
const maxAmount = defineModel<string>('maxAmount', { required: true })
const react = defineModel<boolean>('react', { required: true })
const emit = defineEmits<{ focus: [nodeId: string]; dropRedundant: [] }>()

const { t, locale } = useI18n()
const issueText = (issue: GraphIssue) => (locale.value.startsWith('ru') ? issue.text_ru : issue.text_en)
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-4 [overflow-wrap:anywhere] [&>*]:min-w-0" data-tour="workflow-settings">
    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
      <h3 class="font-semibold">{{ t('workflow.settings.title') }}</h3>
      <p class="text-sm text-muted-foreground">{{ t('workflow.settings.hint') }}</p>
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
      <Label for="workflow-description">{{ t('workflow.settings.description') }}</Label>
      <Textarea id="workflow-description" v-model="description" rows="2" />
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
      <Label for="workflow-cap">{{ t('workflow.settings.cap', { base: props.base.toUpperCase() }) }}</Label>
      <Input id="workflow-cap" v-model="maxAmount" inputmode="decimal" :placeholder="t('workflow.settings.noCap')" />
      <p class="text-xs text-muted-foreground">{{ t('workflow.settings.capHint') }}</p>
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
      <div class="flex items-center justify-between gap-2">
        <Label for="workflow-react">{{ t('workflow.settings.react') }}</Label>
        <Switch id="workflow-react" v-model="react" />
      </div>
      <p class="text-xs text-muted-foreground">{{ t('workflow.settings.reactHint') }}</p>
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0" data-tour="workflow-issues">
      <h4 class="text-sm font-medium">{{ t('workflow.settings.issues') }}</h4>
      <ul v-if="props.issues.length" class="grid grid-cols-[minmax(0,1fr)] gap-1 [&>*]:min-w-0">
        <li v-for="(issue, index) in props.issues" :key="index">
          <Button
            variant="ghost"
            size="block"
            class="px-2 py-1 text-xs w-full rounded-md px-2 py-1 text-left text-xs hover:bg-muted"
            :class="issue.level === 'error' ? 'text-destructive' : 'text-muted-foreground'"
            :disabled="!issue.node_id"
            @click="issue.node_id && emit('focus', issue.node_id)"
          >
            {{ issueText(issue) }}
          </Button>
        </li>
      </ul>
      <p v-else class="text-xs text-muted-foreground">{{ t('workflow.settings.noIssues') }}</p>
      <Button v-if="props.redundant" variant="outline" size="sm" class="justify-self-start" @click="emit('dropRedundant')">
        {{ t('workflow.settings.dropRedundant', { count: props.redundant }) }}
      </Button>
    </div>
  </div>
</template>
