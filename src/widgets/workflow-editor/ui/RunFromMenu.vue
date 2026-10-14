<script setup lang="ts">
import type { Component } from 'vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'

import type { TriggerChoice } from '../model/trigger-choices'

/**
 * «Эмулировать» / «Запустить» with the trigger to start from: the button
 * starts the default one («Запуск вручную» when there is one), the arrow
 * lists every trigger of the workflow.
 */
defineOptions({ inheritAttrs: false })

const props = defineProps<{
  label: string
  icon: Component
  triggers: TriggerChoice[]
  disabled?: boolean
  title?: string
}>()
const emit = defineEmits<{ run: [triggerId: string | null] }>()

const { t } = useI18n()
const first = computed(() => props.triggers[0] ?? null)
const several = computed(() => props.triggers.length > 1)
const buttonTitle = computed(
  () => props.title ?? (first.value ? t('workflow.runFromDefault', { trigger: first.value.title }) : undefined),
)
</script>

<template>
  <div class="inline-flex">
    <Button
      variant="outline"
      :class="several && 'rounded-r-none'"
      :disabled="props.disabled"
      :title="buttonTitle"
      v-bind="$attrs"
      @click="emit('run', first?.id ?? null)"
    >
      <component :is="props.icon" class="size-4" />
      {{ props.label }}
    </Button>
    <DropdownMenu v-if="several" align="end">
      <template #trigger>
        <Button
          variant="outline"
          size="icon"
          class="rounded-l-none border-l-0"
          :disabled="props.disabled"
          :aria-label="t('workflow.runFrom', { action: props.label })"
          :title="t('workflow.runFrom', { action: props.label })"
        >
          <ChevronDown class="size-4" />
        </Button>
      </template>
      <p class="px-2 py-1 text-xs text-muted-foreground">{{ t('workflow.runFrom', { action: props.label }) }}</p>
      <DropdownMenuItem v-for="choice in props.triggers" :key="choice.id" :value="choice.id" @select="(id) => emit('run', id)">
        <span class="grid min-w-0 leading-tight">
          <span class="truncate">{{ choice.title }}</span>
          <span v-if="choice.detail" class="truncate text-xs text-muted-foreground">{{ choice.detail }}</span>
        </span>
      </DropdownMenuItem>
    </DropdownMenu>
  </div>
</template>
