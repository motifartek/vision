/** Saniyeyi `mm:ss` ya da `mm:ss.d` biçimine çevirir. */
export function formatTime(seconds: number, withTenths = false) {
  const safe = Math.max(0, seconds)
  const mm = String(Math.floor(safe / 60)).padStart(2, "0")
  const ss = Math.floor(safe % 60)
  if (!withTenths) return `${mm}:${String(ss).padStart(2, "0")}`
  const tenth = Math.floor((safe % 1) * 10)
  return `${mm}:${String(ss).padStart(2, "0")}.${tenth}`
}
