"use client"

import { useState, useTransition } from "react"
import {
  flexRender,
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  SortingState,
  useReactTable,
} from "@tanstack/react-table"
import { Shield, ShieldAlert, UserX, MoreHorizontal, ArrowUpDown, Loader2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Identity } from "@ory/client"
import { toggleUserState, createTestUser } from "@/app/(protected)/(admin)/users/actions"

interface UsersViewProps {
  users: Identity[]
}

export function UsersView({ users }: UsersViewProps) {
  const [sorting, setSorting] = useState<SortingState>([])
  const [isPending, startTransition] = useTransition()

  const handleToggleState = (userId: string, currentState: string) => {
    startTransition(async () => {
      const res = await toggleUserState(userId, currentState)
      if (res.error) {
        alert(res.error)
      }
    })
  }

  const table = useReactTable({
    data: users,
    columns: [
      {
        accessorKey: "traits.name.first",
        header: ({ column }: any) => {
          return (
            <Button
              variant="ghost"
              onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
            >
              Ad Soyad
              <ArrowUpDown className="ml-2 h-4 w-4" />
            </Button>
          )
        },
        cell: ({ row }: any) => {
          const first = row.original.traits?.name?.first || ""
          const last = row.original.traits?.name?.last || ""
          return <div className="font-medium px-4">{first} {last}</div>
        },
      },
      {
        accessorKey: "traits.email",
        header: "E-posta",
        cell: ({ row }: any) => <div className="lowercase">{row.original.traits?.email}</div>,
      },
      {
        accessorKey: "state",
        header: "Durum",
        cell: ({ row }: any) => {
          const state = row.getValue("state") as string
          return (
            <Badge variant={state === "active" ? "default" : "destructive"}>
              {state === "active" ? "Aktif" : "Askıda"}
            </Badge>
          )
        },
      },
      {
        id: "actions",
        enableHiding: false,
        cell: ({ row }: any) => {
          const user = row.original
          return (
            <DropdownMenu>
              <DropdownMenuTrigger render={<Button variant="ghost" className="h-8 w-8 p-0"><span className="sr-only">Menüyü aç</span><MoreHorizontal className="h-4 w-4" /></Button>} />
              <DropdownMenuContent align="end">
                <DropdownMenuGroup>
                  <DropdownMenuLabel>İşlemler</DropdownMenuLabel>
                  <DropdownMenuItem onClick={() => navigator.clipboard.writeText(user.id)}>
                    ID'yi Kopyala
                  </DropdownMenuItem>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuItem>Görüntüle</DropdownMenuItem>
                  <DropdownMenuItem 
                    disabled={isPending}
                    onClick={() => handleToggleState(user.id, user.state || "active")}
                    className={user.state === "active" ? "text-destructive focus:bg-destructive focus:text-foreground" : "text-green-600 focus:bg-green-600/10"}
                  >
                    {user.state === "active" ? "Askıya Al" : "Aktifleştir"}
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          )
        },
      },
    ],
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    state: {
      sorting,
    },
  })

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Kullanıcı Yönetimi</h1>
          <p className="text-sm text-muted-foreground">Kratos üzerinden kimlikleri yönetin.</p>
        </div>
        <div className="flex gap-2">
          <Button 
            onClick={() => {
              startTransition(async () => {
                const res = await createTestUser()
                if (res.error) alert(res.error)
                else alert("Test kullanıcısı oluşturuldu: " + res.email + " (Şifre: password123)")
              })
            }}
            disabled={isPending}
          >
            {isPending ? <Loader2 className="h-4 w-4 mr-2 animate-spin" /> : <ShieldAlert className="h-4 w-4 mr-2" />}
            Test Kullanıcısı Oluştur
          </Button>
        </div>
      </div>

      <div className="rounded-md border bg-card">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  return (
                    <TableHead key={header.id}>
                      {header.isPlaceholder
                        ? null
                        : flexRender(
                            header.column.columnDef.header,
                            header.getContext()
                          )}
                    </TableHead>
                  )
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows?.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow key={row.id}>
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext()
                      )}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell
                  colSpan={4}
                  className="h-24 text-center"
                >
                  Kullanıcı bulunamadı.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div className="flex items-center justify-end space-x-2 py-4">
        <div className="space-x-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
          >
            Önceki
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
          >
            Sonraki
          </Button>
        </div>
      </div>
    </div>
  )
}
