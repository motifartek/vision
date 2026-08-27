"use client"

import { useCallback, useEffect, useMemo, useState } from "react"
import { Eye, Loader2, Lock, RotateCcw, Save } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

/**
 * Prompt konsolu.
 *
 * Modele giden metnin düzenlendiği yer. İki kural arayüze doğrudan yansıyor:
 *
 * - **Çıktı sözleşmesi düzenlenemez.** `editable: false` parçalar kilitli ve
 *   salt okunur; şema bozulursa şartnamenin puanladığı çıktının kendisi
 *   kırılır.
 * - **Gömülü katalog doğruluk kaynağıdır.** Buradaki düzenlemeler onun
 *   *üstüne biner*; "Varsayılana dön" her zaman bir tık uzakta.
 */

const VISION = process.env.NEXT_PUBLIC_VISION_API ?? "/api/vision"

type Fragment = {
  fragment: string
  editable: boolean
  embedded: string
  override: { text: string; author: string; updated_at: string } | null
}

type Preview = {
  prefix: string
  suffix: string
  version: { number: number; hash: string; source: unknown }
  text_tokens: number
}

/** Parçanın modele giden metni: override varsa o, yoksa gömülü. */
function etkinMetin(f: Fragment) {
  return f.override?.text ?? f.embedded
}

export function PromptsView() {
  const [fragments, setFragments] = useState<Fragment[]>([])
  const [secili, setSecili] = useState<string | null>(null)
  const [taslak, setTaslak] = useState("")
  const [preview, setPreview] = useState<Preview | null>(null)
  const [durum, setDurum] = useState<"bos" | "yukleniyor" | "kaydediliyor" | "hazir">("yukleniyor")
  const [hata, setHata] = useState<string | null>(null)

  const yukle = useCallback(async () => {
    setDurum("yukleniyor")
    try {
      const r = await fetch(`${VISION}/v1/prompts`)
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      const d = (await r.json()) as { fragments: Fragment[] }
      setFragments(d.fragments)
      setHata(null)
    } catch {
      setHata("Görüntü ajanına ulaşılamıyor.")
      setFragments([])
    } finally {
      setDurum("hazir")
    }
  }, [])

  useEffect(() => {
    void yukle()
  }, [yukle])

  const aktif = useMemo(
    () => fragments.find((f) => f.fragment === secili) ?? null,
    [fragments, secili],
  )

  // Parça değişince taslak o parçanın etkin metniyle başlar.
  useEffect(() => {
    if (aktif) setTaslak(etkinMetin(aktif))
  }, [aktif])

  const kirli = aktif != null && taslak !== etkinMetin(aktif)

  const onizle = useCallback(async () => {
    try {
      const r = await fetch(`${VISION}/v1/prompts/preview`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ duration_ms: 35_000 }),
      })
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      setPreview((await r.json()) as Preview)
      setHata(null)
    } catch {
      setHata("Önizleme alınamadı.")
    }
  }, [])

  const kaydet = useCallback(async () => {
    if (!aktif) return
    setDurum("kaydediliyor")
    try {
      const r = await fetch(
        `${VISION}/v1/prompts/vision/${encodeURIComponent(aktif.fragment)}`,
        {
          method: "PUT",
          headers: { "content-type": "application/json; charset=utf-8" },
          body: JSON.stringify({ text: taslak, author: "panel" }),
        },
      )
      if (!r.ok) {
        // Servis reddetme sebebini yazıyor; kullanıcıya onu göster.
        const govde = (await r.json().catch(() => null)) as { error?: string } | null
        throw new Error(govde?.error ?? `HTTP ${r.status}`)
      }
      setHata(null)
      await yukle()
      await onizle()
    } catch (e) {
      setHata((e as Error).message)
    } finally {
      setDurum("hazir")
    }
  }, [aktif, taslak, yukle, onizle])

  const varsayilana_don = useCallback(async () => {
    if (!aktif) return
    setDurum("kaydediliyor")
    try {
      const r = await fetch(
        `${VISION}/v1/prompts/vision/${encodeURIComponent(aktif.fragment)}`,
        { method: "DELETE" },
      )
      if (!r.ok) throw new Error(`HTTP ${r.status}`)
      setHata(null)
      await yukle()
      await onizle()
    } catch (e) {
      setHata((e as Error).message)
    } finally {
      setDurum("hazir")
    }
  }, [aktif, yukle, onizle])

  return (
    <div className="flex w-full flex-col gap-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Prompt&apos;lar</h1>
        <p className="text-muted-foreground">
          Modele giden metin. Düzenlemeler gömülü katalogun üstüne biner; kaynak her zaman depodur.
        </p>
      </div>

      {hata && (
        <div className="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {hata}
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-[260px_minmax(0,1fr)]">
        {/* parça listesi */}
        <div className="flex flex-col gap-1 rounded-xl border bg-card p-2">
          {durum === "yukleniyor" && fragments.length === 0 && (
            <p className="px-2 py-3 text-sm text-muted-foreground">Yükleniyor…</p>
          )}
          {fragments.map((f) => (
            <button
              key={f.fragment}
              type="button"
              onClick={() => setSecili(f.fragment)}
              className={`flex items-center justify-between gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                secili === f.fragment ? "bg-accent" : "hover:bg-accent/60"
              }`}
            >
              <span className="truncate font-mono text-[12px]">{f.fragment}</span>
              <span className="flex shrink-0 items-center gap-1.5">
                {f.override && (
                  <Badge variant="secondary" className="h-4 px-1 text-[9px]">
                    düzenli
                  </Badge>
                )}
                {!f.editable && <Lock className="size-3 text-muted-foreground" />}
              </span>
            </button>
          ))}
        </div>

        {/* düzenleyici */}
        <div className="flex min-w-0 flex-col gap-4">
          {!aktif ? (
            <div className="rounded-xl border bg-card px-4 py-8 text-center text-sm text-muted-foreground">
              Soldan bir parça seçin.
            </div>
          ) : (
            <>
              <div className="flex flex-col gap-3 rounded-xl border bg-card px-4 py-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="font-mono text-sm">{aktif.fragment}</span>
                  {aktif.override ? (
                    <span className="text-[11px] text-muted-foreground">
                      {aktif.override.author} ·{" "}
                      {new Date(aktif.override.updated_at).toLocaleString("tr")}
                    </span>
                  ) : (
                    <span className="text-[11px] text-muted-foreground">gömülü varsayılan</span>
                  )}
                </div>

                {!aktif.editable ? (
                  <>
                    <p className="flex items-start gap-2 text-[12px] leading-snug text-muted-foreground">
                      <Lock className="mt-0.5 size-3.5 shrink-0" />
                      Bu parça çıktı sözleşmesini tarif ediyor ve ayrıştırıcı ona bağlı.
                      Değiştirilirse rapor okunamaz hâle gelir; bu yüzden salt okunur.
                    </p>
                    <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-3 font-mono text-[11px] leading-snug">
                      {aktif.embedded}
                    </pre>
                  </>
                ) : (
                  <>
                    <Textarea
                      value={taslak}
                      onChange={(e) => setTaslak(e.target.value)}
                      rows={10}
                      className="font-mono text-[12px] leading-snug"
                      aria-label={`${aktif.fragment} metni`}
                    />
                    <div className="flex flex-wrap items-center gap-2">
                      <Button size="sm" onClick={kaydet} disabled={!kirli || durum === "kaydediliyor"}>
                        {durum === "kaydediliyor" ? (
                          <Loader2 data-icon="inline-start" className="animate-spin" />
                        ) : (
                          <Save data-icon="inline-start" />
                        )}
                        Kaydet
                      </Button>
                      <Button size="sm" variant="outline" onClick={onizle}>
                        <Eye data-icon="inline-start" />
                        Önizle
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={varsayilana_don}
                        disabled={!aktif.override || durum === "kaydediliyor"}
                        title="Bu parçanın düzenlemesini siler, gömülü metne döner"
                      >
                        <RotateCcw data-icon="inline-start" />
                        Varsayılana dön
                      </Button>
                      {kirli && (
                        <span className="text-[11px] text-muted-foreground">
                          kaydedilmemiş değişiklik
                        </span>
                      )}
                    </div>
                  </>
                )}
              </div>

              {/* gömülüye karşı fark */}
              {aktif.editable && taslak !== aktif.embedded && (
                <div className="flex flex-col gap-2 rounded-xl border bg-card px-4 py-3">
                  <span className="text-xs font-medium">Gömülü varsayılandan farkı</span>
                  <Fark eski={aktif.embedded} yeni={taslak} />
                </div>
              )}

              {preview && (
                <div className="flex flex-col gap-2 rounded-xl border bg-card px-4 py-3">
                  <div className="flex items-baseline justify-between gap-2">
                    <span className="text-xs font-medium">
                      Modele gidecek metin
                      <span className="ml-1 font-normal text-muted-foreground">
                        — ön ek videodan önce, son ek sonra
                      </span>
                    </span>
                    <span className="font-mono text-[10px] text-muted-foreground">
                      v{preview.version.number} · {preview.version.hash} · {preview.text_tokens} token
                    </span>
                  </div>
                  <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-3 font-mono text-[11px] leading-snug">
                    {preview.prefix}
                    {preview.suffix ? `\n\n──────────\n${preview.suffix}` : ""}
                  </pre>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}

/**
 * Satır bazlı fark.
 *
 * Tam bir diff algoritması yerine satır kümesi karşılaştırması: prompt
 * parçaları kısa ve düzenlemeler çoğunlukla cümle ekleme/çıkarma oluyor.
 * Amaç neyin değiştiğini göstermek, sürüm kontrolü yapmak değil.
 */
function Fark({ eski, yeni }: { eski: string; yeni: string }) {
  const a = eski.split("\n")
  const b = yeni.split("\n")
  const aSet = new Set(a)
  const bSet = new Set(b)

  return (
    <div className="overflow-x-auto rounded-md bg-muted/40 p-2 font-mono text-[11px] leading-snug">
      {a
        .filter((s) => !bSet.has(s))
        .map((s, i) => (
          <div key={`e${i}`} className="whitespace-pre-wrap text-destructive">
            − {s || "(boş satır)"}
          </div>
        ))}
      {b
        .filter((s) => !aSet.has(s))
        .map((s, i) => (
          <div key={`y${i}`} className="whitespace-pre-wrap text-emerald-600 dark:text-emerald-400">
            + {s || "(boş satır)"}
          </div>
        ))}
    </div>
  )
}
