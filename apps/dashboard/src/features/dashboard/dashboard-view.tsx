"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import {
  ArrowUpRight,
  Clock3,
  Film,
  HardDrive,
  Play,
  ServerCog,
  Sparkles,
} from "lucide-react";
import { AppShell } from "@/components/app-shell/app-shell";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { formatTime } from "@/features/video-detail/time";

/**
 * Kontrol paneli **gerçek veriyi** gösterir.
 *
 * Eskiden buradaki sayılar (24 proje, 18,4 saat, %91) ve "son projeler" kartları
 * uydurmaydı; üstelik üç kart da var olmayan `/videos/podcast-highlight-03`
 * adresine gidiyordu, yani demoda ilk tıklama kırık bir analiz sayfası açıyordu.
 */
const API = process.env.NEXT_PUBLIC_AUDIO_API ?? "/api/inference";
const STREAM = process.env.NEXT_PUBLIC_STREAM_API ?? "/api/stream";

type VideoEntry = {
  id: string;
  filename: string;
  size: number;
  duration_sec: number | null;
};

type Health = {
  model: {
    name: string;
    weights: string;
    providers: string[];
    classes: number;
  };
  default_profile: string;
};

function formatSize(bytes: number) {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function prettifyName(id: string) {
  return id
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toLocaleUpperCase("tr"));
}

export function DashboardView() {
  const [videos, setVideos] = useState<VideoEntry[] | null>(null);
  const [health, setHealth] = useState<Health | null>(null);
  const [offline, setOffline] = useState(false);

  useEffect(() => {
    let cancelled = false;

    // Video listesi görüntü servisinden; ses servisinin model kartı **isteğe
    // bağlı**. Önceden ikisi de aynı `Promise.all` içindeydi ve ses servisi
    // kapalıyken ana sayfa tümüyle "çevrimdışı" görünüyordu, oysa görsel taraf
    // çalışıyor.
    Promise.resolve(
      fetch(`${STREAM}/v1/videos`).then((r) =>
        r.ok
          ? (r.json() as Promise<{
              videos: {
                id: string;
                original_name: string;
                info: { size_bytes: number; duration_ms: number };
              }[];
            }>)
          : Promise.reject(),
      ),
    )
      .then(({ videos }) => {
        if (cancelled) return;
        setVideos(
          videos.map((v) => ({
            id: v.id,
            filename: v.original_name,
            size: v.info.size_bytes,
            duration_sec: v.info.duration_ms ? v.info.duration_ms / 1000 : null,
          })),
        );
        setOffline(false);

        // Ses servisi ayrı ve gecikmeli; ulaşılamazsa model kartı gizleniyor.
        fetch(`${API}/healthz`)
          .then((r) =>
            r.ok ? (r.json() as Promise<Health>) : Promise.reject(),
          )
          .then((status) => !cancelled && setHealth(status))
          .catch(() => undefined);
      })
      .catch(() => {
        if (!cancelled) setOffline(true);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const totalSeconds = (videos ?? []).reduce(
    (sum, v) => sum + (v.duration_sec ?? 0),
    0,
  );
  const totalBytes = (videos ?? []).reduce((sum, v) => sum + v.size, 0);
  const withDuration = (videos ?? []).filter(
    (v) => v.duration_sec !== null,
  ).length;

  const stats = [
    {
      label: "Kütüphanedeki video",
      value: videos ? String(videos.length) : "—",
      note: offline ? "servise ulaşılamıyor" : "medya kökünde bulunan dosyalar",
      icon: Film,
    },
    {
      label: "Toplam süre",
      value: videos ? formatTime(totalSeconds) : "—",
      // Süre kapsayıcı başlığından okunuyor; bazı dosyalarda yazmıyor.
      note:
        videos && withDuration < videos.length
          ? `${videos.length - withDuration} dosyada süre okunamadı`
          : "kapsayıcı başlığından",
      icon: Clock3,
    },
    {
      label: "Disk kullanımı",
      value: videos ? formatSize(totalBytes) : "—",
      note: "apps/dashboard/public/media",
      icon: HardDrive,
    },
  ];

  const recent = (videos ?? []).slice(0, 3);

  return (
    <div className="mx-auto flex max-w-7xl flex-col gap-6">
      <section className="flex flex-col justify-between gap-4 rounded-xl border bg-card p-6 md:flex-row md:items-center">
        <div className="flex flex-col gap-2">
          <Badge variant="secondary" className="w-fit">
            <Sparkles /> Ses olay analizi
          </Badge>
          <h2 className="max-w-2xl text-balance text-2xl font-semibold md:text-3xl">
            Kayıttaki alarmı, çığlığı ve makine sesini zaman damgasıyla bulun.
          </h2>
          <p className="max-w-xl text-pretty text-sm leading-6 text-muted-foreground">
            Bir video seçin; ses kanalı 527 AudioSet sınıfına göre çözümlensin,
            iş güvenliği kuralları incelenmesi gereken anları işaretlesin.
          </p>
        </div>
        <Button nativeButton={false} render={<Link href="/videos" />} size="lg">
          <Play data-icon="inline-start" /> Video seç
        </Button>
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        {stats.map((stat) => (
          <Card key={stat.label}>
            <CardHeader>
              <CardDescription>{stat.label}</CardDescription>
              <CardAction>
                <stat.icon className="size-4 text-muted-foreground" />
              </CardAction>
              <CardTitle className="text-2xl tabular-nums">
                {stat.value}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xs text-muted-foreground">{stat.note}</p>
            </CardContent>
          </Card>
        ))}
      </section>

      <section className="grid gap-6 lg:grid-cols-[1.5fr_1fr]">
        <Card>
          <CardHeader>
            <CardTitle>Kütüphaneden</CardTitle>
            <CardDescription>Analiz için hazır kayıtlar</CardDescription>
            <CardAction>
              <Button
                variant="ghost"
                size="sm"
                nativeButton={false}
                render={<Link href="/videos" />}
              >
                Tümünü gör <ArrowUpRight data-icon="inline-end" />
              </Button>
            </CardAction>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            {offline ? (
              <p className="py-6 text-center text-xs text-muted-foreground">
                Analiz servisine ulaşılamıyor; kütüphane okunamadı.
              </p>
            ) : !videos ? (
              <p className="py-6 text-center text-xs text-muted-foreground">
                Yükleniyor…
              </p>
            ) : recent.length === 0 ? (
              <p className="py-6 text-center text-xs text-muted-foreground">
                Henüz video yüklenmemiş.
              </p>
            ) : (
              recent.map((video) => (
                <Link
                  href={`/videos/${encodeURIComponent(video.id)}`}
                  key={video.id}
                  className="flex items-center gap-3 rounded-lg border bg-background p-4 transition-colors hover:bg-accent"
                >
                  <div className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-muted">
                    <Film className="size-5 text-primary" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {prettifyName(video.id)}
                    </p>
                    <p className="truncate text-xs text-muted-foreground">
                      {video.filename}
                    </p>
                  </div>
                  <Badge variant="outline" className="shrink-0 tabular-nums">
                    {video.duration_sec === null
                      ? formatSize(video.size)
                      : formatTime(video.duration_sec)}
                  </Badge>
                </Link>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Analiz servisi</CardTitle>
            <CardDescription>Yüklü model ve çalışma zamanı</CardDescription>
            <CardAction>
              <ServerCog className="size-4 text-muted-foreground" />
            </CardAction>
          </CardHeader>
          <CardContent className="flex flex-col gap-3 text-sm">
            {offline || !health ? (
              <div className="flex flex-col gap-2">
                <Badge variant="destructive" className="w-fit">
                  {offline ? "Çevrimdışı" : "Bekleniyor"}
                </Badge>
                <p className="text-xs text-muted-foreground">
                  {offline
                    ? "Servis kapalı görünüyor. Başlatmak için: tools/scripts/start.ps1"
                    : "Durum bilgisi alınıyor…"}
                </p>
              </div>
            ) : (
              <>
                <Badge variant="secondary" className="w-fit">
                  Çalışıyor
                </Badge>
                <dl className="flex flex-col gap-2 text-xs">
                  <div className="flex justify-between gap-3">
                    <dt className="text-muted-foreground">Model</dt>
                    <dd className="truncate font-mono">{health.model.name}</dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="text-muted-foreground">Ağırlık</dt>
                    <dd className="truncate font-mono">
                      {health.model.weights}
                    </dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="text-muted-foreground">Sağlayıcı</dt>
                    <dd className="truncate font-mono">
                      {health.model.providers.join(", ")}
                    </dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="text-muted-foreground">Sınıf</dt>
                    <dd className="font-mono tabular-nums">
                      {health.model.classes}
                    </dd>
                  </div>
                  <div className="flex justify-between gap-3">
                    <dt className="text-muted-foreground">Varsayılan profil</dt>
                    <dd className="font-mono">{health.default_profile}</dd>
                  </div>
                </dl>
              </>
            )}
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
