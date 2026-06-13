import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'

const stubs = {
  LoginView: { template: '<div>LoginView</div>' },
  WelcomeView: { template: '<div>WelcomeView</div>' },
  ChatsView: { template: '<div>ChatsView</div>' },
  SpecsView: { template: '<div>SpecsView</div>' },
  WikiView: { template: '<div>WikiView</div>' },
  RelayView: { template: '<div>RelayView</div>' },
  AgentsConfigView: { template: '<div>AgentsConfigView</div>' },
  ProfessionsView: { template: '<div>ProfessionsView</div>' },
  SkillsView: { template: '<div>SkillsView</div>' },
  ApiSourcesView: { template: '<div>ApiSourcesView</div>' },
  ExplorerView: { template: '<div>ExplorerView</div>' },
  SettingsMenu: { template: '<div>SettingsMenu</div>' },
}

function createTestI18n() {
  return createI18n({
    legacy: false,
    locale: 'en',
    fallbackLocale: 'en',
    messages: { en: {} },
    missing: () => '',
  })
}

async function mountApp(storedView: string | null) {
  const localStorage = globalThis.localStorage as any
  localStorage.getItem.mockReturnValue(storedView)

  const { default: AppComponent } = await import('@/App.vue')
  return mount(AppComponent, {
    global: {
      plugins: [createTestI18n()],
      stubs,
    },
  }) as any
}

describe('App View Persistence Integration', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
    })
    vi.resetModules()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('TC-101: restores specs view after refresh', async () => {
    const wrapper = await mountApp('specs')
    expect(wrapper.vm.currentView).toBe('specs')
  })

  it('TC-102: restores agents view after refresh', async () => {
    const wrapper = await mountApp('agents')
    expect(wrapper.vm.currentView).toBe('agents')
  })

  it('TC-103: restores wiki view after refresh', async () => {
    const wrapper = await mountApp('wiki')
    expect(wrapper.vm.currentView).toBe('wiki')
  })

  it('TC-104: restores explorer view after refresh (with project open)', async () => {
    const wrapper = await mountApp('explorer')
    expect(wrapper.vm.currentView).toBe('explorer')
  })

  it('TC-105: defaults to chats when no stored state', async () => {
    const wrapper = await mountApp(null)
    expect(wrapper.vm.currentView).toBe('chats')
  })

  it('TC-106: preserves view across multiple rapid navigation changes', async () => {
    const localStorage = globalThis.localStorage as any
    const wrapper = await mountApp(null)

    wrapper.vm.currentView = 'specs'
    await wrapper.vm.$nextTick()

    wrapper.vm.currentView = 'agents'
    await wrapper.vm.$nextTick()

    wrapper.vm.currentView = 'wiki'
    await wrapper.vm.$nextTick()

    wrapper.vm.currentView = 'specs'
    await wrapper.vm.$nextTick()

    expect(localStorage.setItem).toHaveBeenCalledWith('autoforge-current-view', 'specs')
    expect(wrapper.vm.currentView).toBe('specs')
  })

  it('TC-107: handles invalid stored data gracefully', async () => {
    const localStorage = globalThis.localStorage as any
    const wrapper = await mountApp('invalid-view-id')

    expect(wrapper.vm.currentView).toBe('chats')
    expect(localStorage.removeItem).toHaveBeenCalledWith('autoforge-current-view')
  })
})
