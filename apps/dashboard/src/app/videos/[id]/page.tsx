import { VideoDetailView } from "@/features/video-detail/video-detail-view"

// Rota kimliği doğrudan medya dosyasını seçer: /videos/test3 → test3.mp4.
// Gateway de aynı eşlemeyi kullanıyor (apps/gateway/src/audio.rs).
export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params
  return <VideoDetailView videoId={id} />
}
