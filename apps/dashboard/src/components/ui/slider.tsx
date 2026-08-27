import { Slider as SliderPrimitive } from "@base-ui/react/slider"

import { cn } from "@/lib/utils"

function Slider({
  className,
  defaultValue,
  value,
  min = 0,
  max = 100,
  ...props
}: SliderPrimitive.Root.Props) {
  const _values = Array.isArray(value)
    ? value
    : Array.isArray(defaultValue)
      ? defaultValue
      : [min, max]

  return (
    <SliderPrimitive.Root
      className={cn("data-horizontal:w-full data-vertical:h-full", className)}
      data-slot="slider"
      defaultValue={defaultValue}
      value={value}
      min={min}
      max={max}
      // `thumbAlignment` bilerek verilmiyor; base-ui varsayılanı `center`.
      //
      // Burada `edge` yazıyordu ve tutamak hiç görünmüyordu: `edge`, tutamağı
      // yerleştirmek için genişliğini **ölçüyor** (kaynakta `inset:
      // thumbAlignment !== 'center'`). Eşik kaydırıcısı `keepMounted` olan bir
      // sekmenin içinde, yani sayfa açılırken `display: none` iken mount
      // oluyor; gizli elemanda ölçüm sıfır çıkıyor ve tutamak konumlanamıyor.
      // Track ölçüm istemediği için çiziliyordu — ekranda dümdüz bir çizgi
      // kalıyor, sürüklenecek bir şey olmuyordu.
      //
      // `center` saf yüzde hesabıyla çalışır, ölçüme ihtiyaç duymaz; sekme
      // gizliyken mount edilse de doğru yerde çıkar.
      {...props}
    >
      <SliderPrimitive.Control className="relative flex w-full touch-none items-center select-none data-disabled:opacity-50 data-vertical:h-full data-vertical:min-h-40 data-vertical:w-auto data-vertical:flex-col">
        <SliderPrimitive.Track
          data-slot="slider-track"
          className="relative grow overflow-hidden rounded-full bg-muted select-none data-horizontal:h-1 data-horizontal:w-full data-vertical:h-full data-vertical:w-1"
        >
          <SliderPrimitive.Indicator
            data-slot="slider-range"
            className="bg-primary select-none data-horizontal:h-full data-vertical:w-full"
          />
        </SliderPrimitive.Track>
        {Array.from({ length: _values.length }, (_, index) => (
          <SliderPrimitive.Thumb
            data-slot="slider-thumb"
            key={index}
            // `after:content-['']` şart: Tailwind v4 `after:` sınıflarına
            // içerik eklemiyor, dolayısıyla `after:-inset-2` ile büyütülmek
            // istenen tıklama alanı hiç oluşmuyordu. Tutamak 12 piksel; onu
            // fareyle yakalamak gereksiz yere zordu.
            className="relative block size-3 shrink-0 rounded-full border border-ring bg-white ring-ring/50 transition-[color,box-shadow] select-none after:absolute after:-inset-2 after:content-[''] hover:ring-3 focus-visible:ring-3 focus-visible:outline-hidden active:ring-3 disabled:pointer-events-none disabled:opacity-50"
          />
        ))}
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  )
}

export { Slider }
