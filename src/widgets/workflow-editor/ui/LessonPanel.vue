<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleCheck, FlaskConical, GraduationCap, Lightbulb } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { type Lesson, WORKFLOW_LESSONS, markLessonDone } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'

/**
 * A lesson beside the canvas: the task, a checklist that ticks as the graph
 * grows, a hint, and the emulation that closes it — all on a copy of the data.
 */
const props = defineProps<{ lesson: Lesson; graph: WorkflowGraph; report: RunReport | null; checking: boolean }>()
const emit = defineEmits<{ check: []; next: [id: string] }>()

const { t } = useI18n()
const hint = ref(false)
const results = computed(() =>
  props.lesson.checks.map((check) => ({ key: check.key, done: check.test(props.graph, props.report) })),
)
const finished = computed(() => results.value.every((result) => result.done))
const next = computed(() => {
  const index = WORKFLOW_LESSONS.findIndex((entry) => entry.id === props.lesson.id)
  return WORKFLOW_LESSONS[index + 1] ?? null
})

watch(finished, (done) => {
  if (!done) return
  markLessonDone(props.lesson.id)
  toast.success(t('workflow.lesson.done'), { description: t(`workflow.lesson.${props.lesson.id}.title`) })
})
</script>

<template>
  <section class="grid gap-3 rounded-lg border border-primary/40 bg-card p-3" data-tour="workflow-lesson">
    <header class="flex items-start gap-2">
      <GraduationCap class="mt-0.5 size-4 shrink-0 text-primary" />
      <div class="grid gap-0.5">
        <h3 class="text-sm font-semibold">{{ t(`workflow.lesson.${props.lesson.id}.title`) }}</h3>
        <p class="text-sm text-muted-foreground">{{ t(`workflow.lesson.${props.lesson.id}.goal`) }}</p>
      </div>
    </header>
    <ol class="grid gap-1">
      <li v-for="result in results" :key="result.key" class="flex items-start gap-2 text-sm">
        <CircleCheck v-if="result.done" class="mt-0.5 size-4 shrink-0 text-primary" />
        <span v-else class="mt-0.5 size-4 shrink-0 rounded-full border border-border" />
        <span :class="result.done ? 'text-muted-foreground line-through' : ''">
          {{ t(`workflow.lesson.${props.lesson.id}.check.${result.key}`) }}
        </span>
      </li>
    </ol>
    <p v-if="hint" class="rounded-md bg-muted p-2 text-xs">{{ t(`workflow.lesson.${props.lesson.id}.hint`) }}</p>
    <div class="flex flex-wrap gap-2">
      <Button size="sm" variant="outline" :disabled="props.checking" @click="emit('check')">
        <FlaskConical class="size-4" />
        {{ t('workflow.lesson.check') }}
      </Button>
      <Button size="sm" variant="ghost" :aria-pressed="hint" @click="hint = !hint">
        <Lightbulb class="size-4" />
        {{ t('workflow.lesson.hint') }}
      </Button>
      <Button v-if="finished && next" size="sm" @click="emit('next', next.id)">{{ t('workflow.lesson.next') }}</Button>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('workflow.lesson.safe') }}</p>
  </section>
</template>
