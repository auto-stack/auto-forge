import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

describe('useViewState', () => {
  const mockLocalStorage = {
    getItem: vi.fn(),
    setItem: vi.fn(),
    removeItem: vi.fn(),
  }

  beforeEach(() => {
    vi.stubGlobal('localStorage', mockLocalStorage)
    // Reset module cache to clear singleton state
    vi.resetModules()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('loads valid view from localStorage', async () => {
    mockLocalStorage.getItem.mockReturnValue('specs')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('specs')
  })

  it('defaults to chats on invalid stored value', async () => {
    mockLocalStorage.getItem.mockReturnValue('invalid-view')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('chats')
    expect(mockLocalStorage.removeItem).toHaveBeenCalledWith('autoforge-current-view')
  })

  it('defaults to chats when localStorage is empty', async () => {
    mockLocalStorage.getItem.mockReturnValue(null)
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('chats')
  })

  it('defaults to chats when localStorage throws', async () => {
    mockLocalStorage.getItem.mockImplementation(() => {
      throw new Error('Private browsing')
    })
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('chats')
  })

  it('saves view changes to localStorage', async () => {
    mockLocalStorage.getItem.mockReturnValue(null)
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    
    currentView.value = 'wiki'
    await new Promise(resolve => setTimeout(resolve, 0))  // Flush watch
    
    expect(mockLocalStorage.setItem).toHaveBeenCalledWith('autoforge-current-view', 'wiki')
  })

  it('handles save errors gracefully', async () => {
    mockLocalStorage.getItem.mockReturnValue(null)
    mockLocalStorage.setItem.mockImplementation(() => {
      throw new Error('Quota exceeded')
    })
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    
    expect(() => {
      currentView.value = 'agents'
    }).not.toThrow()
  })

  it('validates all valid view IDs - chats', async () => {
    mockLocalStorage.getItem.mockReturnValue('chats')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('chats')
  })

  it('validates all valid view IDs - specs', async () => {
    mockLocalStorage.getItem.mockReturnValue('specs')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('specs')
  })

  it('validates all valid view IDs - wiki', async () => {
    mockLocalStorage.getItem.mockReturnValue('wiki')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('wiki')
  })

  it('validates all valid view IDs - agents', async () => {
    mockLocalStorage.getItem.mockReturnValue('agents')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('agents')
  })

  it('validates all valid view IDs - explorer', async () => {
    mockLocalStorage.getItem.mockReturnValue('explorer')
    const { useViewState } = await import('@/composables/useViewState')
    const { currentView } = useViewState()
    expect(currentView.value).toBe('explorer')
  })
})
