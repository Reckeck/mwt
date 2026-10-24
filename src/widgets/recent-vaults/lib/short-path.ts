/** Home directory prefixes of the three desktop systems; the user name is not known here. */
const HOME_PREFIX = /^(?:\/home\/[^/]+|\/Users\/[^/]+|[A-Za-z]:[\\/]Users[\\/][^\\/]+)(?=[\\/]|$)/

/**
 * Directory of a vault file for a list row: «~/Документы/Семья» instead of
 * «/home/anna/Документы/Семья/семья.wallet». With `bare` the leading «~/» goes too (phone sheet).
 */
export function shortDir(path: string, bare = false): string {
  const slash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  const dir = slash > 0 ? path.slice(0, slash) : slash === 0 ? '/' : ''
  const short = dir.replace(HOME_PREFIX, '~').replaceAll('\\', '/')
  return bare ? short.replace(/^~\//, '') : short
}
