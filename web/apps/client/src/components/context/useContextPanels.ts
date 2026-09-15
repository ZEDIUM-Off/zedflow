import { computed, onMounted, onUnmounted, ref, watch, type Ref } from 'vue'

export function useContextPanels(layout: Ref<HTMLElement | undefined>) {
  const source = ref(345), preview = ref(525), width = ref(1500)
  let customized = false
  try {
    const saved = JSON.parse(localStorage.getItem('zedflow.context-panels') || 'null')
    if (Number.isFinite(saved?.source)) source.value = Math.max(230, Math.min(440, saved.source))
    if (Number.isFinite(saved?.preview)) preview.value = Math.max(280, Math.min(600, saved.preview))
    customized = Number.isFinite(saved?.source) && Number.isFinite(saved?.preview)
  } catch { /* A missing or obsolete preference uses the reference layout. */ }
  const sizes = computed(() => {
    const available = Math.max(510, width.value - 350)
    const factor = Math.min(1, available / (source.value + preview.value))
    return { source: Math.max(230, source.value * factor), preview: Math.max(280, preview.value * factor) }
  })
  const style = computed(() => ({ '--ctx-source-width': `${sizes.value.source}px`, '--ctx-preview-width': `${sizes.value.preview}px` }))
  function remember() { customized = true; try { localStorage.setItem('zedflow.context-panels', JSON.stringify({ source: source.value, preview: preview.value })) } catch { /* The current layout still works if storage is unavailable. */ } }
  function set(side: 'source' | 'preview', value: number) {
    const target = side === 'source' ? source : preview
    target.value = Math.max(side === 'source' ? 230 : 280, Math.min(side === 'source' ? 440 : 600, value))
  }
  function pointer(event: PointerEvent, side: 'source' | 'preview') {
    if (event.button !== 0) return
    event.preventDefault()
    const handle = event.currentTarget as HTMLElement, start = event.clientX, initial = sizes.value[side]
    handle.setPointerCapture(event.pointerId)
    const move = (current: PointerEvent) => set(side, initial + (current.clientX - start) * (side === 'source' ? 1 : -1))
    const end = () => { handle.removeEventListener('pointermove', move); handle.removeEventListener('pointerup', end); handle.removeEventListener('lostpointercapture', end); remember() }
    handle.addEventListener('pointermove', move); handle.addEventListener('pointerup', end); handle.addEventListener('lostpointercapture', end)
  }
  function keyboard(event: KeyboardEvent, side: 'source' | 'preview') {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    const value = side === 'source' ? source.value : preview.value
    set(side, event.key === 'Home' ? 0 : event.key === 'End' ? 1000 : value + (event.key === 'ArrowRight' ? 16 : -16) * (side === 'source' ? 1 : -1)); remember()
  }
  let observer: ResizeObserver | undefined
  onMounted(() => {
    observer = new ResizeObserver(entries => {
      if (!entries[0]?.contentRect.width) return
      width.value = entries[0].contentRect.width
      if (!customized) { source.value = Math.max(230, Math.min(440, Math.round(width.value * .23))); preview.value = Math.max(280, Math.min(600, Math.round(width.value * .35))) }
    })
    if (layout.value) observer.observe(layout.value)
  })
  onUnmounted(() => observer?.disconnect())
  watch(layout, (element, previous) => {
    if (previous) observer?.unobserve(previous)
    if (element) observer?.observe(element)
  }, { flush: 'post' })
  return { sizes, style, pointer, keyboard }
}
