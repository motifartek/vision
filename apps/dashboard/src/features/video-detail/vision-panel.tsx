"use client"

import { useState } from "react"
import { ChevronDown, Loader2, Play, Sparkles } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { formatTime } from "./time"
import { streamUrl, type AgentStep, type Outcome, type Payload } from "./vision-analysis"

/**
 * Görsel analiz sonucu — sistemin nihai çıktısı.
 *
 * Şartname §5 dört alan istiyor: özet, zaman damgalı olaylar, risk ve aksiyon
 * önerileri. Panelin bu bölümü onları birebir gösteriyor; altındaki katlanır
 * bölüm ise **modele tam olarak ne gittiğini** açıyor.
 */
export function VisionPanel({
  outcome,
  payload,
  running,
  error,
  onAnalyze,
  onLoadPayload,
  onSeek,
  ready,
}: {
  outcome: Outcome | null
  payload: Payload | null
  running: boolean
  error: string | null
  onAnalyze: () => void
  onLoadPayload: () => void
  onSeek: (seconds: number) => void
  /** Video görüntü servisinde bulunamadıysa analiz düğmesi anlamsız. */
  ready: boolean
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
      <div className="flex shrink-0 flex-col gap-2.5 rounded-xl border bg-card px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm font-medium">Görsel analiz</span>
          {outcome?.report.processing_ms != null && (
            <span className="font-mono text-[10px] text-muted-foreground tabular-nums">
              {(outcome.report.processing_ms / 1000).toFixed(1)} sn
            </span>
          )}
        </div>

        <Button size="sm" onClick={onAnalyze} disabled={!ready || running}>
          {running ? (
            <Loader2 data-icon="inline-start" className="animate-spin" />
          ) : (
            <Sparkles data-icon="inline-start" />
          )}
          {running ? "Analiz ediliyor…" : outcome ? "Yeniden analiz et" : "Videoyu analiz et"}
        </Button>

        {!ready && (
          <p className="text-[11px] leading-snug text-muted-foreground">
            Bu video görüntü servisine yüklenmemiş. Yeniden yükleyin.
          </p>
        )}
        {error && <p className="text-[11px] leading-snug text-destructive">{error}</p>}
      </div>

      {outcome && <Report outcome={outcome} onSeek={onSeek} />}

      <PayloadSection payload={payload} onLoad={onLoadPayload} ready={ready} />
    </div>
  )
}

const RISK_RENK: Record<string, string> = {
  Yüksek: "bg-destructive/15 text-destructive border-destructive/30",
  Orta: "bg-amber-500/15 text-amber-600 border-amber-500/30 dark:text-amber-400",
  Düşük: "bg-emerald-500/15 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
}

function Report({ outcome, onSeek }: { outcome: Outcome; onSeek: (s: number) => void }) {
  const { report, steps } = outcome

  return (
    <>
      <div className="flex shrink-0 flex-col gap-2.5 rounded-xl border bg-card px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs text-muted-foreground">Genel risk</span>
          <Badge variant="outline" className={RISK_RENK[report.risk] ?? ""}>
            {report.risk}
          </Badge>
        </div>
        <p className="text-[13px] leading-relaxed">{report.summary}</p>
      </div>

      <div className="flex shrink-0 flex-col gap-2 rounded-xl border bg-card px-4 py-3">
        <div className="flex items-baseline justify-between">
          <span className="text-xs font-medium">Olaylar</span>
          <span className="font-mono text-[10px] text-muted-foreground tabular-nums">
            {report.events.length}
          </span>
        </div>
        {report.events.length === 0 ? (
          <p className="text-[11px] text-muted-foreground">Olay bildirilmedi.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {report.events.map((ev, i) => (
              <li key={`${ev.t_ms}-${i}`}>
                <button
                  type="button"
                  onClick={() => onSeek(ev.t_ms / 1000)}
                  className="flex w-full gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent focus-visible:bg-accent focus-visible:outline-none"
                  title="O ana git"
                >
                  <span className="shrink-0 font-mono text-[11px] tabular-nums text-muted-foreground">
                    {ev.time}
                  </span>
                  <span className="min-w-0 flex-1 text-[12px] leading-snug">{ev.event}</span>
                  <span
                    className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full"
                    style={{ background: nokta(ev.severity) }}
                    title={ev.severity}
                  />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex shrink-0 flex-col gap-2 rounded-xl border bg-card px-4 py-3">
        <span className="text-xs font-medium">Aksiyon önerileri</span>
        <ul className="flex flex-col gap-1.5">
          {report.actions.map((a, i) => (
            <li key={i} className="flex gap-2 text-[12px] leading-snug">
              <span className="mt-[7px] h-1 w-1 shrink-0 rounded-full bg-foreground/50" />
              {a}
            </li>
          ))}
        </ul>
      </div>

      <Steps steps={steps} />
    </>
  )
}

function nokta(s: string) {
  if (s === "Yüksek") return "rgb(220,70,60)"
  if (s === "Orta") return "rgb(230,160,50)"
  return "rgb(80,180,130)"
}

/**
 * Ajanın attığı adımlar.
 *
 * Şartname çıktıların açıklanabilir olmasını istiyor. Model kararının nereden
 * geldiğini görmenin tek yolu, hangi pencereye hangi hızda baktığını bilmek.
 */
function Steps({ steps }: { steps: AgentStep[] }) {
  if (steps.length === 0) return null

  return (
    <div className="flex shrink-0 flex-col gap-1.5 rounded-xl border bg-card px-4 py-3">
      <span className="text-xs font-medium">Ajan adımları</span>
      <ol className="flex flex-col gap-1">
        {steps.map((s) => (
          <li key={s.step} className="flex items-baseline gap-2 text-[11px]">
            <span className="font-mono text-muted-foreground tabular-nums">{s.step + 1}</span>
            <span className="flex-1 truncate">
              {s.action === "report" ? "raporladı" : s.action}
              <span className="text-muted-foreground">
                {" · "}
                {formatTime(s.t0_ms / 1000)}–{formatTime(s.t1_ms / 1000)}
                {s.time_scale > 1.01 && ` · ${s.time_scale.toFixed(0)}× ağır çekim`}
                {` · ${s.service_frames} kare`}
              </span>
            </span>
            <span className="font-mono text-muted-foreground tabular-nums">
              {(s.elapsed_ms / 1000).toFixed(1)}s
            </span>
          </li>
        ))}
      </ol>
    </div>
  )
}

/**
 * Modele tam olarak ne gidiyor.
 *
 * Sistemin en kolay gözden kaçan sorusu bu. Klibin kendisi oynatılabiliyor,
 * yanında ağır çekim oranı ve token maliyeti duruyor; yani burada görülen şey
 * temsili bir gösterim değil, modele gidenin birebir aynısı.
 */
function PayloadSection({
  payload,
  onLoad,
  ready,
}: {
  payload: Payload | null
  onLoad: () => void
  ready: boolean
}) {
  const [open, setOpen] = useState(false)

  return (
    <div className="flex shrink-0 flex-col rounded-xl border bg-card">
      <button
        type="button"
        onClick={() => {
          const yeni = !open
          setOpen(yeni)
          if (yeni && !payload && ready) onLoad()
        }}
        className="flex items-center justify-between gap-2 px-4 py-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
        aria-expanded={open}
      >
        <span className="text-xs font-medium">Modele giden yük</span>
        <ChevronDown
          className={`size-3.5 text-muted-foreground transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <div className="flex flex-col gap-3 border-t px-4 py-3">
          {!payload ? (
            <p className="text-[11px] text-muted-foreground">
              {ready ? "Klip hazırlanıyor…" : "Video görüntü servisinde yok."}
            </p>
          ) : (
            <>
              {/* Klibin kendisi. Ham video değil, tam olarak bu gidiyor. */}
              <video
                src={streamUrl(payload.clip.url)}
                controls
                playsInline
                className="w-full rounded-md bg-black"
              />

              <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-[11px]">
                <Row
                  k="Kesilen aralık"
                  v={`${formatTime(payload.clip.t0_ms / 1000)} – ${formatTime(payload.clip.t1_ms / 1000)}`}
                />
                <Row
                  k="Hız"
                  v={
                    payload.clip.time_scale > 1.01
                      ? `${payload.clip.time_scale.toFixed(1)}× ağır çekim`
                      : "gerçek zaman"
                  }
                />
                <Row k="Servisin göreceği kare" v={`${payload.clip.service_frames} (2 fps)`} />
                <Row k="Efektif fps" v={payload.clip.effective_fps.toFixed(1)} />
                <Row k="Klip boyutu" v={`${(payload.clip.size_bytes / 1e6).toFixed(1)} MB`} />
                <Row k="Azaltma" v={`${payload.reduction.ratio.toFixed(0)}×`} />
                <Row
                  k="Token (tahmini)"
                  v={payload.tokens.total.toLocaleString("tr")}
                  vurgu={payload.tokens.total > 12000}
                />
                <Row
                  k="Kare boyutu"
                  v={`${payload.tokens.frame_width}×${payload.tokens.frame_height}`}
                />
              </dl>

              {payload.clip.time_scale > 1.01 && (
                <p className="text-[10px] leading-snug text-muted-foreground">
                  Kaynakta {(payload.clip.source_span_ms / 1000).toFixed(1)} sn olan aralık{" "}
                  {(payload.clip.duration_ms / 1000).toFixed(1)} sn&apos;ye yayıldı. Servis sabit 2 fps
                  örneklediği için modelin verdiği zamanlar klibin saatiyle gelir; kaynağa çeviriyi
                  sistem yapar.
                </p>
              )}

              <div className="flex flex-col gap-1">
                <span className="text-[10px] font-medium text-muted-foreground">İstem</span>
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-2 font-mono text-[10px] leading-snug">
                  {payload.prompt}
                </pre>
              </div>

              {payload.evidence_frames.length > 0 && (
                <div className="flex flex-col gap-1">
                  <span className="text-[10px] font-medium text-muted-foreground">
                    Aralığı seçtiren anlar
                    <span className="font-normal"> — modele gitmiyor</span>
                  </span>
                  <div className="flex gap-1 overflow-x-auto pb-1">
                    {payload.evidence_frames.map((f) => (
                      <img
                        key={f.ord}
                        src={streamUrl(f.url)}
                        alt={f.time}
                        title={`${f.time} · hareket ${f.motion_score.toFixed(2)}`}
                        loading="lazy"
                        className={`h-12 shrink-0 rounded-sm ${f.is_scene_cut ? "ring-1 ring-[rgb(160,140,255)]" : ""}`}
                      />
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  )
}

function Row({ k, v, vurgu }: { k: string; v: string; vurgu?: boolean }) {
  return (
    <>
      <dt className="text-muted-foreground">{k}</dt>
      <dd className={`text-right font-mono tabular-nums ${vurgu ? "text-amber-600 dark:text-amber-400" : ""}`}>
        {v}
      </dd>
    </>
  )
}
