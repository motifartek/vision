"use client"

import { AlertTriangle, ShieldCheck, TriangleAlert } from "lucide-react"
import type { Analysis, SafetyFinding } from "./audio-analysis"
import { formatTime } from "./time"

type Props = {
  analysis: Analysis | null
  onSeek: (seconds: number, autoplay?: boolean) => void
}

const TONE: Record<string, { ring: string; text: string; bg: string }> = {
  critical: { ring: "var(--destructive)", text: "var(--destructive)", bg: "color-mix(in oklab, var(--destructive) 12%, transparent)" },
  warning: { ring: "#F5A524", text: "#F5A524", bg: "color-mix(in oklab, #F5A524 12%, transparent)" },
  info: { ring: "var(--muted-foreground)", text: "var(--muted-foreground)", bg: "transparent" },
}

/**
 * İş güvenliği bulguları.
 *
 * Bulgular ihlal kararı değildir: tek mikrofonda yön bilgisi olmadığı için
 * sistem kimin nerede durduğunu bilemez. Panel bu yüzden "incele" dilini
 * kullanır ve her bulgunun kanıtını (hangi olaylar tetikledi) gösterir.
 */
export function SafetyPanel({ analysis, onSeek }: Props) {
  const findings = analysis?.safety?.findings ?? []
  const eventCount = analysis?.safety?.events.length ?? 0

  if (!analysis) return null

  return (
    <section className="flex min-h-0 flex-col gap-2 rounded-xl border bg-card p-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 text-sm font-semibold">
          {findings.length > 0 ? (
            <TriangleAlert className="size-4" style={{ color: "var(--destructive)" }} />
          ) : (
            <ShieldCheck className="size-4 text-success" />
          )}
          İş güvenliği
        </h2>
        <span className="font-mono text-[11px] text-muted-foreground">
          {eventCount} olay · {findings.length} bulgu
        </span>
      </div>

      {findings.length === 0 ? (
        <p className="py-3 text-xs text-muted-foreground">
          Bu kayıtta incelenmesi gereken bulgu yok. {eventCount > 0 && `${eventCount} güvenlik sınıfı sesi (makine, araç, insan) olağan seviyede.`}
        </p>
      ) : (
        <div className="flex min-h-0 flex-col gap-1.5 overflow-auto">
          {findings.map((f, i) => (
            <FindingRow key={i} finding={f} onSeek={onSeek} />
          ))}
        </div>
      )}
    </section>
  )
}

function FindingRow({ finding, onSeek }: { finding: SafetyFinding; onSeek: (s: number, a?: boolean) => void }) {
  const tone = TONE[finding.severity] ?? TONE.info
  return (
    <button
      type="button"
      onClick={() => onSeek(finding.start_sec, true)}
      className="flex flex-col gap-1 rounded-md border px-2.5 py-2 text-left transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
      style={{ borderColor: tone.ring, background: tone.bg }}
      title={finding.detail}
    >
      <div className="flex items-center gap-2">
        <AlertTriangle className="size-3 shrink-0" style={{ color: tone.text }} />
        <span className="min-w-0 flex-1 truncate text-xs font-medium">{finding.title}</span>
        <span className="shrink-0 font-mono text-[10px] tabular-nums" style={{ color: tone.text }}>
          {formatTime(finding.start_sec)}
        </span>
      </div>
      {/* Kanıt: kuralı hangi olaylar tetikledi — bulgunun denetlenebilir olması için */}
      <span className="line-clamp-2 text-[10px] leading-snug text-muted-foreground">
        {finding.evidence.join(" · ")}
      </span>
    </button>
  )
}
