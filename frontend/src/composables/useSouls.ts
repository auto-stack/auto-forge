import { ref } from 'vue'

export interface SoulDto {
  id: string
  name: string
  markdown: string
}

const _souls = ref<SoulDto[]>([])
const _soulMap = ref<Record<string, SoulDto>>({})

export function useSouls() {
  async function loadSouls() {
    try {
      const resp = await fetch('/api/forge/relay/souls')
      if (!resp.ok) return
      const data = await resp.json()
      _souls.value = data.souls ?? []
      const map: Record<string, SoulDto> = {}
      for (const s of _souls.value) {
        map[s.id] = s
      }
      _soulMap.value = map
    } catch {}
  }

  function getSoul(id: string): SoulDto | undefined {
    return _soulMap.value[id]
  }

  function getSoulMarkdown(id: string): string {
    return _soulMap.value[id]?.markdown ?? ''
  }

  return { souls: _souls, soulMap: _soulMap, loadSouls, getSoul, getSoulMarkdown }
}
