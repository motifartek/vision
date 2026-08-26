import { VideoDetailView } from "@/features/video-detail/video-detail-view"

// Rota kimliği uzantısız: /videos/test3 → medya kökündeki `test3.*`. Uzantıyı
// inference servisi çözüyor (upload::find_by_id); gateway de aynı kimliği
// uzantı eklemeden iletiyor (apps/gateway/src/audio.rs).
export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params
  return <VideoDetailView videoId={id} />
}
