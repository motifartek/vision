/**
 * Saniyeyi `mm:ss`, `mm:ss.d` ya da bir saati aşıyorsa `s:mm:ss` biçimine çevirir.
 *
 * Saat alanı şart: güvenlik kamerası kayıtları saatler sürüyor ve saatsiz biçim
 * orada `75:30` gibi okunmaz değerler veriyordu. Sunucu tarafındaki karşılığı
 * `safety.rs::ts`.
 */
export function formatTime(seconds: number, withTenths = false) {
  const safe = Math.max(0, seconds)
  const hours = Math.floor(safe / 3600)
  const mm = String(Math.floor((safe % 3600) / 60)).padStart(2, "0")
  const ss = String(Math.floor(safe % 60)).padStart(2, "0")
  const head = hours > 0 ? `${hours}:${mm}` : mm
  if (!withTenths) return `${head}:${ss}`
  const tenth = Math.floor((safe % 1) * 10)
  return `${head}:${ss}.${tenth}`
}
