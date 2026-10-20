<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { CircleCheck, GraduationCap } from 'lucide-vue-next'

import { WORKFLOW_LESSONS, lessonsDone } from '../../model/lessons'

/** The node lessons, in order, with the ones finished on this device ticked. */
const { t } = useI18n()
const done = ref<string[]>([])
onMounted(() => {
  done.value = lessonsDone()
})
</script>

<template>
  <section class="grid gap-2" data-tour="workflow-lessons">
    <h3 class="flex items-center gap-1.5 text-sm font-medium">
      <GraduationCap class="size-4" />
      {{ t('workflow.lesson.listTitle', { done: done.length, total: WORKFLOW_LESSONS.length }) }}
    </h3>
    <ol class="grid gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">
      <li v-for="(lesson, index) in WORKFLOW_LESSONS" :key="lesson.id">
        <RouterLink
          :to="{ name: 'workflow-new', query: { lesson: lesson.id } }"
          class="flex items-start gap-2 rounded-lg border border-border p-3 transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <CircleCheck v-if="done.includes(lesson.id)" class="mt-0.5 size-4 shrink-0 text-primary" />
          <span v-else class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border border-border text-2xs">
            {{ index + 1 }}
          </span>
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t(`workflow.lesson.${lesson.id}.title`) }}</span>
            <span class="text-xs text-muted-foreground">{{ t(`workflow.lesson.${lesson.id}.goal`) }}</span>
          </span>
        </RouterLink>
      </li>
    </ol>
  </section>
</template>
