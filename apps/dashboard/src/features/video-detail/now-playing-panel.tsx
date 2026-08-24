"use client"

import { useMemo } from "react"
import { AudioLines, Gauge, Radio } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import type { Analysis, AnalysisSource } from "./audio-analysis"
import { frameAt } from "./frame-lookup"
import { formatTime } from "./time"

type Props = {
  analysis: Analysis | null
  source: AnalysisSource
  currentTime: number
  threshold: number
  nameOf: (index: number) => string
}

/**
 * Videonun o anki saniyesinde duyulan sesleri canlı listeler — altyazı gibi.
 * Panelin işi okumak, sıralamak değil; bu yüzden en fazla 6 satır gösterir.
 */
export function NowPlayingPanel({ analysis, source, currentTime, threshold, nameOf }: Props) {
  const rows = useMemo(() => {
    // Pencere `[t, t+window]` aralığını anlatıyor; merkezi hizala, yoksa
    // etiketler videonun yarım pencere gerisinde kalır.
    const centerOffset = (analysis?.model.window_sec ?? 0) / 2
    const frame = frameAt(analysis?.frames, currentTime, centerOffset)
    if (!frame) return []
    return frame.top.map(([index, score]) => ({ index, score, name: nameOf(index) }))
  }, [analysis, currentTime, nameOf])

  const hasFrames = Boolean(analysis?.frames?.length)

  return (
    <aside className="flex min-h-0 flex-col gap-3 rounded-xl border bg-card p-4">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <AudioLines className="size-4 text-primary" /> Şu an duyulan
          </h2>
          <p className="mt-0.5 font-mono text-xs tabular-nums text-muted-foreground">
            {formatTime(currentTime, true)}
          </p>
        </div>
        {source === "live" ? (
          <Badge variant="secondary" className="shrink-0"><Radio /> Canlı</Badge>
        ) : source === "error" ? (
          <Badge variant="destructive" className="shrink-0">Analiz yok</Badge>
        ) : (
          <Badge variant="outline" className="shrink-0">Yükleniyor…</Badge>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-auto">
        {source === "error" ? (
          /* "Bekleniyor" demek burada yanlış: gelmeyecek bir şeyi bekletiyor. */
          <p className="py-6 text-center text-xs text-muted-foreground">
            Bu videonun sesi analiz edilemedi.
          </p>
        ) : !analysis ? (
          <p className="py-6 text-center text-xs text-muted-foreground">Analiz bekleniyor…</p>
        ) : !hasFrames ? (
          <p className="py-6 text-center text-xs text-muted-foreground">
            Saniye bazlı veri yok. Analiz servisini başlatın.
          </p>
        ) : rows.length === 0 ? (
          <p className="py-6 text-center text-xs text-muted-foreground">Bu anda ses yok.</p>
        ) : (
          rows.map((row) => {
            const active = row.score >= threshold
            return (
              <div
                key={row.index}
                className="flex items-center gap-2.5 rounded-md px-2 py-1.5 transition-colors"
                style={{ background: active ? "color-mix(in oklab, var(--primary) 10%, transparent)" : undefined }}
              >
                <span
                  className="size-1.5 shrink-0 rounded-full"
                  style={{ background: active ? "var(--primary)" : "var(--muted-foreground)", opacity: active ? 1 : 0.4 }}
                  aria-hidden
                />
                <span className={`min-w-0 flex-1 truncate text-xs ${active ? "font-medium" : "text-muted-foreground"}`}>
                  {row.name}
                </span>
                {/* Skor hem sayı hem çubuk: göz çubuğu, kayıt sayıyı okur */}
                <span className="h-1 w-10 shrink-0 overflow-hidden rounded-full bg-muted">
                  <span
                    className="block h-full rounded-full bg-primary transition-[width] duration-150"
                    style={{ width: `${Math.round(row.score * 100)}%`, opacity: active ? 1 : 0.45 }}
                  />
                </span>
                <span className="w-8 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted-foreground">
                  %{Math.round(row.score * 100)}
                </span>
              </div>
            )
          })
        )}
      </div>

      {analysis && (
        <div className="flex flex-wrap items-center gap-1.5 border-t pt-3">
          <Badge variant="outline" className="text-[10px]">
            <Gauge /> {analysis.timing.realtime_factor.toFixed(0)}× gerçek zaman
          </Badge>
          {/* Kırpma sessiz kalırsa "bu kayıtta başka bir şey yok" diye okunuyor;
              artı işareti listenin tam olmadığını söylüyor. Güvenlik olayları
              kırpmadan muaf, yani bulgular eksilmiyor. */}
          <Badge
            variant="outline"
            className="text-[10px]"
            title={
              analysis.events_truncated
                ? "Olay listesi sınıra takıldığı için kırpıldı; güvenlik sınıfları kırpmadan muaf tutuldu."
                : undefined
            }
          >
            {analysis.events.length}
            {analysis.events_truncated ? "+" : ""} olay
          </Badge>
          <Badge variant="outline" className="text-[10px]">{analysis.model.name}</Badge>
        </div>
      )}
    </aside>
  )
}
