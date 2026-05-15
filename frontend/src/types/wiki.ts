export type WikiSource = 'manual' | 'guide' | 'api_ref' | 'custom'

export interface WikiPage {
  slug: string
  title: string
  content: string
  source_type: WikiSource
  tags: string[]
  version: number
  created_at: number
  updated_at: number
}

export interface WikiPageMeta {
  slug: string
  title: string
  source_type: WikiSource
  tags: string[]
  version: number
  updated_at: number
}

export interface TreeNode {
  name: string
  path: string
  type: 'file' | 'folder'
  children?: TreeNode[]
  size?: number
  modified?: number
}
