/** What the unlock card says after a quick-unlock attempt failed. The reasons for a reset are not told apart. */
export type BiometricNotice = 'failed' | 'reset' | 'password'

const NOTICE_BY_CODE: Record<string, BiometricNotice> = {
  BIOMETRIC_INVALIDATED: 'reset',
  BIOMETRIC_PASSWORD_REQUIRED: 'password',
}

/** Anything that is not a reset or an expired window reads as "did not work": cancelled, not recognised, failed. */
export function noticeFor(code: string): BiometricNotice {
  return NOTICE_BY_CODE[code] ?? 'failed'
}
