import type { ChildProcess } from 'node:child_process'
export function freePort(): Promise<number>
export function startVite(): Promise<{ vite: ChildProcess; url: string }>
