import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { settingsStore } from '@shared/store/settingsStore'

export function useWindowAnimation() {
  useEffect(() => {
    const container = document.querySelector('.main-container')
    if (!container) return

    // 记录当前正在运行的动画 rAF id，启动新动画前取消旧动画，
    // 避免快速 show/hide 时 expand 与 collapse 两个 rAF 循环互相覆盖导致闪跳。
    // 作为 effect 局部变量，天然隔离不同组件实例，无需 useRef。
    let expandRafId = null
    let collapseRafId = null

    function cancelAllRaf() {
      if (expandRafId) {
        cancelAnimationFrame(expandRafId)
        expandRafId = null
      }
      if (collapseRafId) {
        cancelAnimationFrame(collapseRafId)
        collapseRafId = null
      }
    }

    // 展开动画
    function animateExpand(container) {
      // 取消所有正在进行的动画（含未完成的 expand），避免 rAF 泄漏导致并行动画互相覆盖
      cancelAllRaf()

      const duration = 400
      const startTime = performance.now()
      const targetHeight = window.innerHeight - 10

      container.style.height = '0'
      container.style.opacity = '0'
      container.style.overflow = 'hidden'

      function easeWithSettleBounce(t) {
        if (t < 0.7) {
          const normalized = t / 0.7
          return 1 - Math.pow(1 - normalized, 3)
        }

        const settlePhase = (t - 0.7) / 0.3
        const overshoot = Math.cos(settlePhase * Math.PI) * 0.012
        return 1 - overshoot * (1 - settlePhase)
      }

      function animate(currentTime) {
        const progress = Math.min((currentTime - startTime) / duration, 1)
        const eased = easeWithSettleBounce(progress)

        container.style.height = `${targetHeight * eased}px`
        container.style.opacity = Math.min(progress * 2, 1)

        if (progress < 1) {
          expandRafId = requestAnimationFrame(animate)
        } else {
          expandRafId = null
          container.style.height = 'calc(100vh - 10px)'
          container.style.opacity = '1'
        }
      }

      expandRafId = requestAnimationFrame(animate)
    }

    // 收起动画
    function animateCollapse(container) {
      // 取消所有正在进行的动画（含未完成的 collapse），避免 rAF 泄漏导致并行动画互相覆盖
      cancelAllRaf()

      const duration = 200
      const startTime = performance.now()
      // 从当前实际高度开始收起，避免从全高跳变（快速 show/hide 时当前高度可能不是全高）
      // 使用 getComputedStyle 获取计算后的像素高度，兼容 calc() 等非像素单位，
      // 避免 parseFloat('calc(100vh - 10px)') 返回 NaN 而回退到 window.innerHeight 导致首帧 10px 跳变
      const currentHeight = parseFloat(getComputedStyle(container).height)
      const startHeight = Number.isFinite(currentHeight) && currentHeight > 0
        ? currentHeight
        : window.innerHeight

      container.style.overflow = 'hidden'

      function animate(currentTime) {
        const progress = Math.min((currentTime - startTime) / duration, 1)
        const eased = Math.pow(progress, 2)

        container.style.height = `${startHeight * (1 - eased)}px`
        container.style.opacity = 1 - eased

        if (progress < 1) {
          collapseRafId = requestAnimationFrame(animate)
        } else {
          collapseRafId = null
          container.style.height = '0'
          container.style.opacity = '0'
        }
      }

      collapseRafId = requestAnimationFrame(animate)
    }

    const unlistenShow = listen('window-show-animation', () => {
      if (settingsStore.clipboardAnimationEnabled) {
        animateExpand(container)
      } else {
        cancelAllRaf()
        container.style.height = 'calc(100vh - 10px)'
        container.style.opacity = '1'
      }
    })

    const unlistenHide = listen('window-hide-animation', () => {
      if (settingsStore.clipboardAnimationEnabled) {
        animateCollapse(container)
      } else {
        cancelAllRaf()
        container.style.height = '0'
        container.style.opacity = '0'
        container.style.overflow = 'hidden'
      }
    })

    // 超时显示
    const fallbackTimer = setTimeout(() => {
      if (container.style.height !== 'calc(100vh - 10px)') {
        container.style.height = 'calc(100vh - 10px)'
        container.style.opacity = '1'
      }
    }, 200)

    return () => {
      unlistenShow.then(fn => fn())
      unlistenHide.then(fn => fn())
      clearTimeout(fallbackTimer)
      cancelAllRaf()
    }
  }, [])
}
