import { 
  DndContext, 
  closestCenter,
  KeyboardSensor,
  MouseSensor as LibMouseSensor,
  useSensor,
  useSensors,
  DragOverlay,
} from '@dnd-kit/core'
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { restrictToVerticalAxis } from '@dnd-kit/modifiers'
import { useState, useCallback } from 'react'

function shouldHandleDrag(element) {
  let cur = element
  while (cur) {
    if (cur.dataset && cur.dataset.dragIgnore === 'true') {
      return false
    }
    cur = cur.parentElement
  }
  return true
}

class CustomMouseSensor extends LibMouseSensor {
  static activators = [
    {
      eventName: 'onMouseDown',
      handler: ({ nativeEvent: event }) => {
        return shouldHandleDrag(event.target)
      },
    },
  ]
}

// 可排序列表上下文 Hook
export function useSortableList({ items, onDragEnd, restrictToVertical = true }) {
  const [activeId, setActiveId] = useState(null)
  
  const sensors = useSensors(
    useSensor(CustomMouseSensor, {
      activationConstraint: {
        distance: 3,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  )

  // 获取项目
  const getItem = useCallback((id) => {
    return items.find((item) => item._sortId === id)
  }, [items])

  const customCollisionDetection = useCallback((args) => {
    const collisions = closestCenter(args)
    if (!collisions.length) return collisions
    
    const activeItem = getItem(args.active.id)
    if (!activeItem) return collisions
    
    return collisions.filter(collision => {
      const overItem = getItem(collision.id)
      if (!overItem) return true
      return activeItem.is_pinned === overItem.is_pinned
    })
  }, [getItem])

  const handleDragStart = (event) => {
    setActiveId(event.active.id)
  }

  const handleDragEnd = (event) => {
    const { active, over } = event
    setActiveId(null)

    if (over && active.id !== over.id) {
      const oldIndex = items.findIndex((item) =>
         item._sortId === active.id
    )
      const newIndex = items.findIndex((item) =>
         item._sortId === over.id
    )

      if (oldIndex !== -1 && newIndex !== -1 && onDragEnd) {
        onDragEnd(oldIndex, newIndex)
      }
    }
  }
  
  const handleDragCancel = () => {
    setActiveId(null)
  }
  
  // 获取当前拖拽的项
  const activeItem = activeId 
    ? items.find((item) => item._sortId === activeId)
    : null

  return {
    DndContext,
    SortableContext,
    DragOverlay,
    sensors,
    handleDragStart,
    handleDragEnd,
    handleDragCancel,
    activeId,
    activeItem,
    strategy: verticalListSortingStrategy,
    modifiers: restrictToVertical ? [restrictToVerticalAxis] : [],
    collisionDetection: customCollisionDetection,
  }
}

export { useSortable } from '@dnd-kit/sortable'
export { CSS } from '@dnd-kit/utilities'

