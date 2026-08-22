import { redirect } from "next/navigation"

// Kök sayfa, korumalı route grubuna yönlendirir.
export default function RootPage() {
  redirect("/")
}
