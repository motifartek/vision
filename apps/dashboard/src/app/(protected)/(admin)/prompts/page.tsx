import { PromptsView } from "@/features/prompts/prompts-view"

/**
 * Prompt konsolu — yalnızca yönetici.
 *
 * `(admin)` grubunda olması bilinçli: prompt alanı modele doğrudan talimat
 * kanalıdır, yetkisiz erişim sistemin davranışını değiştirebilmek demektir.
 */
export default function PromptsPage() {
  return <PromptsView />
}
