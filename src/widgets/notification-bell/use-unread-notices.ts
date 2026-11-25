import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** The notification journal and how many of its records are unread (shared by the list and the profile menu's dot). */
export function useUnreadNotices() {
  const notices = useQuery({ queryKey: ['notifications'], queryFn: () => call<NoticeRecord[]>('list_notifications') })
  const unread = computed(() => (notices.data.value ?? []).filter((record) => !record.read).length)
  return { notices, unread }
}
