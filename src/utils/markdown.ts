import MarkdownIt from 'markdown-it'
import type StateCore from 'markdown-it/lib/rules_core/state_core.mjs'
import type Token from 'markdown-it/lib/token.mjs'
import katex from '@vscode/markdown-it-katex'

/**
 * Markdown for AI answers: no raw HTML (escaped), KaTeX math, clickable source
 * citations ([S1], 【S2】, [S1, S3]) and badges for the provenance tags
 * 【教材】 / 【补充】 / 【推导】 required by the Tutor Engine prompt.
 */
const PATTERN = /[[【]\s*((?:S\s*\d{1,4}\s*[,，、;；]?\s*)+)[\]】]|【(教材|补充|推导)】/g
const LABELS: Record<string, string> = { 教材: 'textbook', 补充: 'supplement', 推导: 'derived' }

function splitText(state: StateCore, token: Token): Token[] {
  const text = token.content
  const out: Token[] = []
  let last = 0
  for (const match of text.matchAll(PATTERN)) {
    const index = match.index ?? 0
    if (index > last) {
      const plain = new state.Token('text', '', 0)
      plain.content = text.slice(last, index)
      out.push(plain)
    }
    if (match[1]) {
      const cite = new state.Token('examops_cite', '', 0)
      cite.meta = { labels: Array.from(match[1].matchAll(/S\s*(\d{1,4})/g), m => `S${m[1]}`) }
      out.push(cite)
    } else if (match[2]) {
      const label = new state.Token('examops_label', '', 0)
      label.meta = { name: match[2] }
      out.push(label)
    }
    last = index + match[0].length
  }
  if (!out.length) return [token]
  if (last < text.length) {
    const plain = new state.Token('text', '', 0)
    plain.content = text.slice(last)
    out.push(plain)
  }
  return out
}

function sourcePlugin(md: MarkdownIt) {
  md.core.ruler.push('examops_sources', (state) => {
    for (const block of state.tokens) {
      if (block.type !== 'inline' || !block.children) continue
      block.children = block.children.flatMap(child => (child.type === 'text' ? splitText(state, child) : [child]))
    }
  })
  const escape = md.utils.escapeHtml
  md.renderer.rules.examops_cite = (tokens, idx) =>
    (tokens[idx].meta.labels as string[])
      .map(label => `<button type="button" class="cite" data-cite="${escape(label)}">${escape(label)}</button>`)
      .join('')
  md.renderer.rules.examops_label = (tokens, idx) => {
    const name = tokens[idx].meta.name as string
    return `<span class="source-tag source-tag-${LABELS[name]}">${escape(name)}</span>`
  }
  // Links open in the system browser (handled by the click listener), never in the app window.
  const defaultLink = md.renderer.rules.link_open ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options))
  md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
    tokens[idx].attrSet('data-external', '1')
    tokens[idx].attrSet('rel', 'noopener noreferrer')
    return defaultLink(tokens, idx, options, env, self)
  }
}

const md = new MarkdownIt({ html: false, linkify: false, breaks: true, typographer: false })
md.use(katex, { throwOnError: false })
md.use(sourcePlugin)

export function renderMarkdown(text: string): string {
  return md.render(text)
}

/** Source labels cited in an answer (same rules as the backend). */
export function citedLabels(text: string): string[] {
  const labels: string[] = []
  for (const match of text.matchAll(PATTERN)) {
    if (!match[1]) continue
    for (const m of match[1].matchAll(/S\s*(\d{1,4})/g)) {
      const label = `S${m[1]}`
      if (!labels.includes(label)) labels.push(label)
    }
  }
  return labels
}
