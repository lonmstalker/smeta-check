// Общая обёртка страниц входа. Ошибки выводят FormError/FieldError
// из @/components/form.
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export function AuthCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <main className="mx-auto mt-16 max-w-sm p-4">
      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">{children}</CardContent>
      </Card>
    </main>
  )
}
