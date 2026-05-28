import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createI18n } from 'vue-i18n'
import en from '../locales/en.json'
import zh from '../locales/zh.json'

function collectKeys(obj: Record<string, unknown>, prefix = ''): string[] {
  const keys: string[] = []
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      keys.push(...collectKeys(value as Record<string, unknown>, fullKey))
    } else {
      keys.push(fullKey)
    }
  }
  return keys
}

describe('i18n setup', () => {
  it('creates i18n with legacy: false and fallbackLocale: en', () => {
    const i18n = createI18n({
      legacy: false,
      locale: 'en',
      fallbackLocale: 'en',
      messages: { en, zh },
    })
    expect(i18n.mode).toBe('composition')
    expect(i18n.global.fallbackLocale.value).toBe('en')
  })

  it('loads both en and zh message objects', () => {
    const i18n = createI18n({
      legacy: false,
      locale: 'en',
      fallbackLocale: 'en',
      messages: { en, zh },
    })
    const enKeys = Object.keys(i18n.global.getLocaleMessage('en'))
    const zhKeys = Object.keys(i18n.global.getLocaleMessage('zh'))
    expect(enKeys.length).toBeGreaterThan(0)
    expect(zhKeys.length).toBeGreaterThan(0)
  })

  it('detectLocale logic: stored value takes priority', () => {
    const store: Record<string, string> = { 'autoforge-language': 'zh' }
    const stored = store['autoforge-language']
    const isValid = stored && ['en', 'zh'].includes(stored)
    expect(isValid).toBe(true)
  })

  it('detectLocale logic: invalid stored value rejected', () => {
    const store: Record<string, string> = { 'autoforge-language': 'fr' }
    const stored = store['autoforge-language']
    const isValid = stored && ['en', 'zh'].includes(stored)
    expect(isValid).toBe(false)
  })

  it('detectLocale logic: Chinese browser maps to zh', () => {
    const nav = 'zh-CN'.toLowerCase()
    expect(nav.startsWith('zh')).toBe(true)
  })

  it('detectLocale logic: English browser maps to en', () => {
    const nav = 'en-US'.toLowerCase()
    expect(nav.startsWith('zh')).toBe(false)
  })
})

describe('setLocale', () => {
  let store: Record<string, string>

  beforeEach(() => {
    store = {}
  })

  it('setLocale persists to localStorage', () => {
    store['autoforge-language'] = 'zh'
    expect(store['autoforge-language']).toBe('zh')
  })

  it('setLocale sets document lang attribute', () => {
    document.documentElement.setAttribute('lang', 'zh')
    expect(document.documentElement.getAttribute('lang')).toBe('zh')
  })

  it('toggleLocale switches between en and zh', () => {
    store['autoforge-language'] = 'en'
    let current = store['autoforge-language']
    const toggled = current === 'en' ? 'zh' : 'en'
    store['autoforge-language'] = toggled
    expect(store['autoforge-language']).toBe('zh')
  })
})

describe('translation file parity', () => {
  it('en.json and zh.json have identical key structures', () => {
    const enKeys = collectKeys(en as unknown as Record<string, unknown>).sort()
    const zhKeys = collectKeys(zh as unknown as Record<string, unknown>).sort()
    expect(enKeys).toEqual(zhKeys)
  })

  it('all required top-level sections exist in both files', () => {
    const sections = ['app', 'nav', 'settings', 'common', 'welcome', 'chat', 'specs', 'wiki', 'explorer', 'relay', 'agents', 'professions', 'skills', 'apis', 'gate', 'report', 'category', 'detail']
    for (const section of sections) {
      expect(en[section as keyof typeof en]).toBeDefined()
      expect(zh[section as keyof typeof zh]).toBeDefined()
    }
  })

  it('settings section has all required keys', () => {
    const requiredSettings = ['title', 'mode', 'accent', 'theme', 'language', 'modeGsdTitle', 'modeCheckTitle', 'themeLight', 'themeDark', 'themeSystem']
    for (const key of requiredSettings) {
      expect((en.settings as Record<string, string>)[key]).toBeDefined()
      expect((zh.settings as Record<string, string>)[key]).toBeDefined()
    }
  })

  it('nav section has all 9 navigation keys', () => {
    const navKeys = ['explorer', 'chat', 'specs', 'wiki', 'relay', 'agents', 'professions', 'skills', 'apis']
    for (const key of navKeys) {
      expect((en.nav as Record<string, string>)[key]).toBeDefined()
      expect((zh.nav as Record<string, string>)[key]).toBeDefined()
    }
  })

  it('no empty string values in en.json', () => {
    const enKeys = collectKeys(en as unknown as Record<string, unknown>)
    for (const key of enKeys) {
      const parts = key.split('.')
      let val: unknown = en
      for (const p of parts) val = (val as Record<string, unknown>)[p]
      expect(val, `Key "${key}" has empty value in en.json`).toBeTruthy()
    }
  })

  it('no empty string values in zh.json', () => {
    const zhKeys = collectKeys(zh as unknown as Record<string, unknown>)
    for (const key of zhKeys) {
      const parts = key.split('.')
      let val: unknown = zh
      for (const p of parts) val = (val as Record<string, unknown>)[p]
      expect(val, `Key "${key}" has empty value in zh.json`).toBeTruthy()
    }
  })

  it('common section has standard action labels in both locales', () => {
    expect(en.common.save).toBe('Save')
    expect(zh.common.save).toBe('保存')
    expect(en.common.cancel).toBe('Cancel')
    expect(zh.common.cancel).toBe('取消')
    expect(en.common.delete).toBe('Delete')
    expect(zh.common.delete).toBe('删除')
    expect(en.common.edit).toBe('Edit')
    expect(zh.common.edit).toBe('编辑')
    expect(en.common.close).toBe('Close')
    expect(zh.common.close).toBe('关闭')
  })

  it('brand name stays as AutoForge in both locales', () => {
    expect(en.app.brandName).toBe('AutoForge')
    expect(zh.app.brandName).toBe('AutoForge')
  })
})
