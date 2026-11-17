import { useVirtualizer } from "@tanstack/react-virtual"
import type { ReactNode } from "react"
import { useRef } from "react"

import { cn } from "@/lib/utils"

export function VirtualList<T>({
  items,
  getKey,
  label,
  estimateSize,
  renderItem,
}: {
  items: readonly T[]
  getKey: (item: T) => string
  label: string
  estimateSize: number
  renderItem: (item: T) => ReactNode
}) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => estimateSize,
    getItemKey: (index) => {
      const item = items[index]
      return item === undefined ? index : getKey(item)
    },
    overscan: 6,
  })

  return (
    <div ref={scrollRef} className="max-h-[480px] overflow-y-auto">
      <ul
        aria-label={label}
        className="relative"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((row) => {
          const item = items[row.index]
          if (item === undefined) return null
          return (
            <li
              key={row.key}
              ref={virtualizer.measureElement}
              data-index={row.index}
              aria-setsize={items.length}
              aria-posinset={row.index + 1}
              className={cn(
                "absolute top-0 left-0 w-full",
                row.index > 0 && "border-t border-border"
              )}
              style={{ transform: `translateY(${row.start}px)` }}
            >
              {renderItem(item)}
            </li>
          )
        })}
      </ul>
    </div>
  )
}
