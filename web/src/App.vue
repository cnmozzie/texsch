<script setup lang="ts">
import { ref, onMounted, nextTick } from 'vue'
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import init, { compile, generate_stub, get_rotated_footprint } from './wasm/texsch'

self.MonacoEnvironment = {
  getWorker() {
    return new editorWorker()
  },
}

const SEP = '='.repeat(45)

const DEFAULT_INPUT = `\
U1: OPA330xxD
R1: R
R2: R
R3: R
R4: R
J1: Conn_01x03_Socket
J2: Conn_Coaxial
J3: Conn_Coaxial
J4: Conn_Coaxial
#VCC1: VCC
#VSS1: VSS
#GND1: GND
#GND2: GND
#GND3: GND
#GND4: GND
${SEP}

 [in1] --<J3                                #VCC1
          v                                 v
          |                                 |
          ^                                 ^
          #GND3             +--------------<
                            |                U1>----*-----+--[OUT]
 [in2]---<J4                ^         +--- <        |     |
          v                 #GND2     |     v       |     |
          |            [In1]--<R1> -- *     |       |     |
          ^                           |     ^       |     +--<J2
          #GND4        [In2]--<R2> -- *     #VSS1   |         v
                                      |             |         |
                                      +------<R3>---+         ^
 [VCC]----<                                                   #GND1
 [GND]----<J1
 [VSS]----<
${SEP}
`

/** Find 0-based line indices of all `====...` separator lines. */
function findSeparators(text: string): number[] {
  const seps: number[] = []
  const lines = text.split('\n')
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].length >= 3 && /^=+$/.test(lines[i])) {
      seps.push(i)
    }
  }
  return seps
}

/** Check whether a 1-based Monaco line number falls on a separator line. */
function isOnSeparatorLine(lineNumber: number): boolean {
  if (!editor) return false
  const line = editor.getModel()!.getLineContent(lineNumber)
  return line.length >= 3 && /^=+$/.test(line)
}

/** Check whether a 1-based line number is in Grid2 (after the second separator). */
function isInGrid2(lineNumber: number): boolean {
  if (!editor) return false
  const seps = findSeparators(editor.getValue())
  const sep2 = seps[1]
  if (sep2 === undefined) return false
  return lineNumber > sep2 + 1 // 0-based sep2 → 1-based: sep2+1 is the separator line itself
}

/** Pad all non-separator lines to the maximum column width with spaces.
 *  Returns true if any edits were applied. */
function padAllLinesToMaxWidth(): boolean {
  if (!editor) return false
  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // Find max column width across all lines.
  let maxCol = 0
  for (let i = 1; i <= totalLines; i++) {
    const len = model.getLineMaxColumn(i) - 1 // exclude trailing newline
    if (len > maxCol) maxCol = len
  }
  if (maxCol === 0) return false

  // Pad shorter lines that aren't separators.
  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []
  for (let i = 1; i <= totalLines; i++) {
    const line = model.getLineContent(i)
    // Don't pad separator lines — trailing spaces would break ^=+$ detection.
    if (line.length >= 3 && /^=+$/.test(line)) continue
    const len = line.length
    if (len < maxCol) {
      edits.push({
        range: new monaco.Range(i, len + 1, i, len + 1),
        text: ' '.repeat(maxCol - len),
      })
    }
  }

  if (edits.length > 0) {
    suppressPad = true
    editor.executeEdits('pad-lines', edits)
    return true
  }
  return false
}

interface ComponentDef {
  symbol: string
  label: string
  prefix: string
}

const COMPONENT_LIBRARY: ComponentDef[] = [
  { symbol: 'R', label: 'Resistor', prefix: 'R' },
  { symbol: 'C', label: 'Capacitor', prefix: 'C' },
  { symbol: 'L', label: 'Inductor', prefix: 'L' },
  { symbol: 'OPA330xxD', label: 'OpAmp OPA330', prefix: 'U' },
  { symbol: 'GND', label: 'Ground', prefix: '#GND' },
  { symbol: 'VCC', label: 'VCC Power', prefix: '#VCC' },
  { symbol: 'VSS', label: 'VSS Power', prefix: '#VSS' },
  { symbol: 'Conn_Coaxial', label: 'Coaxial Conn', prefix: 'J' },
  { symbol: 'Conn_01x03_Socket', label: '3-pin Header', prefix: 'J' },
]

interface ComponentTextSpan {
  refdes: string
  kind: 'Port' | 'Label'
  line_number: number
  start_col: number
  end_col: number
}

const svgContent = ref('')
const kicadContent = ref('')
const activeTab = ref<'svg' | 'kicad'>('svg')
const wasmReady = ref(false)
const editorContainer = ref<HTMLElement | null>(null)
const activeRefDes = ref<string | null>(null)

let editor: monaco.editor.IStandaloneCodeEditor | null = null
let sourceMapSpans: ComponentTextSpan[] = []
let componentAngles: Record<string, number> = {}
let currentDecorations: string[] = []
let selectionDecorations: string[] = []
let lastInjectedRefdes: string | null = null
let suppressPreviewClear = false
let suppressPad = false
interface BoxRect { minLine: number; maxLine: number; minCol: number; maxCol: number }
let currentBoxSelection: BoxRect | null = null
const activeRefDesList = ref<string[]>([])

onMounted(async () => {
  await init()
  wasmReady.value = true

  await nextTick()

  if (!editorContainer.value) return

  editor = monaco.editor.create(editorContainer.value, {
    value: DEFAULT_INPUT,
    language: 'plaintext',
    theme: 'vs-dark',
    columnSelection: true,
    minimap: { enabled: false },
    fontSize: 14,
    fontFamily:
      "'Cascadia Code', 'Fira Code', 'JetBrains Mono', ui-monospace, Consolas, monospace",
    lineNumbers: 'on',
    scrollBeyondLastLine: false,
    automaticLayout: true,
    wordWrap: 'off',
  })

  editor.onDidChangeModelContent(() => {
    // Padding re-entry: re-compile with padded text but don't strip preview.
    if (suppressPad) {
      suppressPad = false
      const text = editor!.getValue()
      doCompile(text)
      const pos = editor!.getPosition()
      if (pos) {
        matchAndHighlightComponent(pos.lineNumber, pos.column)
      }
      // Run padding once more; should be a no-op if already padded.
      padAllLinesToMaxWidth()
      return
    }

    if (suppressPreviewClear) {
      suppressPreviewClear = false
    } else if (lastInjectedRefdes) {
      // User edited the document — remove the preview zone.
      suppressPreviewClear = true
      removeInjectedComponent()
      return // the edit above will re-trigger this handler for compile
    }

    const text = editor!.getValue()
    doCompile(text)
    const pos = editor!.getPosition()
    if (pos) {
      matchAndHighlightComponent(pos.lineNumber, pos.column)
    }

    // Auto-pad all non-separator lines to max column width.
    padAllLinesToMaxWidth()
  })

  editor.onDidChangeCursorSelection((e) => {
    const sels = editor!.getSelections() ?? [e.selection]
    const hasArea = sels.some(
      s =>
        s.startLineNumber !== s.endLineNumber ||
        s.startColumn !== s.endColumn,
    )

    if (hasArea) {
      handleBoxSelection()
    } else {
      if (activeRefDesList.value.length > 0) {
        const hit = isInProtectedSpan(
          e.selection.startLineNumber,
          e.selection.startColumn,
        )
        if (hit && activeRefDesList.value.includes(hit.refdes)) {
          currentDecorations = editor!.deltaDecorations(currentDecorations, [])
          activeRefDes.value = hit.refdes
          setSvgHighlightRule(null)
          return
        }
      }
      clearBoxSelection()
      matchAndHighlightComponent(
        e.selection.startLineNumber,
        e.selection.startColumn,
      )
    }
  })

  // ---- 2D Canvas Grid Mode: overwrite paste --------------------------
  // We intercept the native paste event at the document level (capture
  // phase) so we get clipboardData synchronously before Monaco ever sees
  // the event.  We intentionally do NOT preventDefault on the Ctrl+V
  // keydown – the browser must fire the paste event for clipboard data to
  // be available.
  const editorDom = editor.getDomNode()
  const docPasteHandler = (e: ClipboardEvent) => {
    if (!editorDom || !editorDom.contains(e.target as Node)) return
    e.preventDefault()
    e.stopPropagation()
    const text = e.clipboardData?.getData('text/plain') ?? ''
    if (text) handleGridPaste(text)
  }
  document.addEventListener('paste', docPasteHandler, true)

  // ---- 2D Canvas Grid Mode: overwrite input + entity protection ------
  editor.onKeyDown((e) => {
    const key = e.browserEvent.key
    const ctrl = e.browserEvent.ctrlKey || e.browserEvent.metaKey
    const alt = e.browserEvent.altKey

    // Let ctrl-combo shortcuts pass through (Ctrl+C, Ctrl+Z, etc.)
    if (ctrl) return

    // Alt shortcuts for component translation & rotation
    if (alt) {
      if (handleAltShortcut(e.browserEvent)) {
        e.preventDefault()
        e.stopPropagation()
        return
      }
      return
    }

    if (key === 'Escape') {
      if (activeRefDesList.value.length > 0) {
        e.preventDefault()
        e.stopPropagation()
        clearBoxSelection()
        // Restore single-component highlight at current cursor position.
        const pos = editor!.getPosition()
        if (pos) {
          matchAndHighlightComponent(pos.lineNumber, pos.column)
        }
        return
      }
      // No box selection active — let Monaco handle Escape normally.
      return
    }

    if (key === 'Enter') {
      e.preventDefault()
      e.stopPropagation()
      const pos = editor!.getPosition()
      if (pos && (isOnSeparatorLine(pos.lineNumber) || isInGrid2(pos.lineNumber))) return
      handleGridEnter()
      return
    }

    if (key === 'Backspace' || key === 'Delete') {
      // Box selection active and Delete pressed → wipe entire rectangle
      // plus remove header declarations.  Allowed in Grid2 too.
      if (!(key === 'Backspace') && activeRefDesList.value.length > 0) {
        e.preventDefault()
        e.stopPropagation()
        deleteBoxSelection()
        return
      }
      // Block individual delete on separator lines and in Grid2.
      const pos = editor!.getPosition()
      if (pos && (isOnSeparatorLine(pos.lineNumber) || isInGrid2(pos.lineNumber))) return
      e.preventDefault()
      e.stopPropagation()
      handleGridDelete(key === 'Backspace')
      return
    }

    // Printable single character
    if (key.length === 1) {
      e.preventDefault()
      e.stopPropagation()
      const pos = editor!.getPosition()
      if (pos && (isOnSeparatorLine(pos.lineNumber) || isInGrid2(pos.lineNumber))) return
      handleGridOverwrite(key)
      return
    }
    // All other keys (arrows, Enter, Tab, Escape, Home, End, PageUp, etc.)
    // pass through to Monaco's default handler unchanged.
  })

  doCompile(DEFAULT_INPUT)
})

function setSvgHighlightRule(refdes: string | null) {
  const styleId = 'svg-component-highlight'
  let styleEl = document.getElementById(styleId)
  if (!styleEl) {
    styleEl = document.createElement('style')
    styleEl.id = styleId
    document.head.appendChild(styleEl)
  }
  if (refdes) {
    styleEl.textContent = `
      .svg-wrapper.highlighted [data-refdes="${refdes}"] :not(text) {
        stroke: rgba(66, 133, 244, 0.5) !important;
        stroke-width: 2.5px !important;
        filter: drop-shadow(0px 0px 4px rgba(66, 133, 244, 0.3));
      }
      .svg-wrapper.highlighted [data-label="${refdes}"] text {
        fill: #4285F4 !important;
      }
    `
  } else {
    styleEl.textContent = ''
  }
}

function matchAndHighlightComponent(line: number, col: number) {
  const hit = sourceMapSpans.find(
    (s) => s.line_number === line && col >= s.start_col && col <= s.end_col,
  )

  if (hit) {
    // Highlight ALL ports of the same component in Monaco, not just the
    // one under the cursor — the user sees every pin light up at once.
    const siblings = sourceMapSpans.filter((s) => s.refdes === hit.refdes)
    const decorations = siblings.map((s) => ({
      range: new monaco.Range(
        s.line_number,
        s.start_col,
        s.line_number,
        s.end_col + 1, // Monaco Range endColumn is exclusive
      ),
      options: {
        inlineClassName: 'monaco-component-active',
      },
    }))
    currentDecorations = editor!.deltaDecorations(currentDecorations, decorations)
    activeRefDes.value = hit.refdes
    setSvgHighlightRule(hit.refdes)
  } else {
    currentDecorations = editor!.deltaDecorations(currentDecorations, [])
    activeRefDes.value = null
    setSvgHighlightRule(null)
  }
}

// ---- Box Selection: SVG multi-highlight with golden glow ----------------
function setSvgMultiHighlight(refdesList: string[]) {
  const styleId = 'svg-multi-highlight'
  let styleEl = document.getElementById(styleId)
  if (!styleEl) {
    styleEl = document.createElement('style')
    styleEl.id = styleId
    document.head.appendChild(styleEl)
  }
  if (refdesList.length > 0) {
    const selectors = refdesList
      .map(r => `.svg-wrapper [data-refdes="${r}"] :not(text)`)
      .join(',\n')
    const labelSelectors = refdesList
      .map(r => `.svg-wrapper [data-label="${r}"] text`)
      .join(',\n')
    styleEl.textContent = `
      ${selectors} {
        stroke: rgba(255, 193, 7, 0.65) !important;
        stroke-width: 3px !important;
        filter: drop-shadow(0px 0px 6px rgba(255, 193, 7, 0.45));
      }
      ${labelSelectors} {
        fill: #FFC107 !important;
      }
    `
  } else {
    styleEl.textContent = ''
  }
}

function deleteBoxSelection() {
  if (!editor || !currentBoxSelection) return
  const { minLine, maxLine, minCol, maxCol } = currentBoxSelection
  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  // 1. Erase every cell in the selection rectangle (set to spaces),
  //    but skip separator lines — they are rigid boundaries.
  for (let line = minLine; line <= maxLine; line++) {
    if (line < 1 || line > totalLines) continue
    // Never modify separator lines.
    if (isOnSeparatorLine(line)) continue
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    if (minCol > oldLen) continue // rectangle entirely right of content
    const endCol = Math.min(maxCol, oldLen)
    const arr = [...oldContent]
    for (let c = minCol; c <= endCol; c++) {
      arr[c - 1] = ' '
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({
        range: new monaco.Range(line, 1, line, oldLen + 1),
        text: newContent,
      })
    }
  }

  // 2. Remove header declarations for every selected refdes.
  const lines = editor.getValue().split('\n')
  const deletedRefdes = new Set(activeRefDesList.value)
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^\s*(\S+)\s*:/)
    if (m && deletedRefdes.has(m[1])) {
      edits.push({ range: new monaco.Range(i + 1, 1, i + 2, 1), text: '' })
    }
  }

  // 3. Clear box selection state.
  clearBoxSelection()
  currentDecorations = editor.deltaDecorations(currentDecorations, [])
  activeRefDes.value = null
  setSvgHighlightRule(null)

  if (edits.length === 0) return

  suppressPreviewClear = true
  editor.executeEdits('box-delete', edits)
  doCompile(editor.getValue())
}

function clearBoxSelection() {
  selectionDecorations = editor!.deltaDecorations(selectionDecorations, [])
  activeRefDesList.value = []
  currentBoxSelection = null
  setSvgMultiHighlight([])
}

function scanBoxSelection() {
  if (!editor) return

  // With columnSelection:true, Monaco natively returns one Selection per
  // line when the user Alt+drags a box selection.  Aggregate them.
  const sels = editor.getSelections()
  if (!sels || sels.length === 0) return

  let minLine = Math.min(...sels.map(s => Math.min(s.startLineNumber, s.endLineNumber)))
  let maxLine = Math.max(...sels.map(s => Math.max(s.startLineNumber, s.endLineNumber)))
  const minCol  = Math.min(...sels.map(s => Math.min(s.startColumn, s.endColumn)))
  const maxCol  = Math.max(...sels.map(s => Math.max(s.startColumn, s.endColumn)))

  // Clamp to grid boundaries: box selection must not span across separator lines.
  // Find which grid the selection starts in and clamp maxLine accordingly.
  const seps = findSeparators(editor.getValue())
  const sepLines = seps.map(s => s + 1) // 0-based → 1-based Monaco
  for (const sep of sepLines) {
    if (minLine < sep && maxLine >= sep) {
      // Spans separator — clamp to the grid containing minLine.
      maxLine = sep - 1
    } else if (minLine >= sep && maxLine < sep) {
      // This shouldn't happen if minLine >= sep.
    }
  }
  // Clamp minLine upward if it's below a separator and maxLine is above a later one.
  for (const sep of sepLines) {
    if (minLine > sep) continue // minLine is below this separator, fine
    if (minLine <= sep && maxLine > sep) {
      maxLine = sep - 1
    }
  }

  currentBoxSelection = { minLine, maxLine, minCol, maxCol }

  // Scan source-map spans for intersected components AND labels.
  const hitRefdes = new Set<string>()
  const hitSpans: ComponentTextSpan[] = []

  for (const span of sourceMapSpans) {
    if (span.line_number < minLine || span.line_number > maxLine) continue
    if (span.start_col > maxCol || span.end_col < minCol) continue
    hitRefdes.add(span.refdes)
    hitSpans.push(span)
  }

  activeRefDesList.value = [...hitRefdes]

  // Monaco decorations: highlight ALL intersected spans (Port + Label).
  const decorations = hitSpans.map(s => ({
    range: new monaco.Range(
      s.line_number, s.start_col, s.line_number, s.end_col + 1,
    ),
    options: { inlineClassName: 'monaco-component-selected' },
  }))
  selectionDecorations = editor.deltaDecorations(selectionDecorations, decorations)

  // SVG multi-highlight
  setSvgMultiHighlight([...hitRefdes])

  // Keep activeRefDes truthy so the SVG wrapper gets .highlighted class,
  // but clear the single-component CSS so it doesn't conflict.
  activeRefDes.value = hitRefdes.size > 0 ? [...hitRefdes][0] : null
  setSvgHighlightRule(null)
}

function handleBoxSelection() {
  // Suppress single-component highlight when entering box selection.
  currentDecorations = editor!.deltaDecorations(currentDecorations, [])
  scanBoxSelection()
}

interface RefdesReassignment {
  old_refdes: string
  new_refdes: string
  symbol_name: string
  positions: [number, number, number][] // [line, start_col, text_width] 1-based
}

function applyRefdesReassignments(json: string) {
  if (!json || json === '[]') return
  if (!editor) return

  let reassignments: RefdesReassignment[]
  try {
    reassignments = JSON.parse(json)
  } catch {
    return
  }
  if (!reassignments || reassignments.length === 0) return

  const fullText = editor.getValue()
  const lines = fullText.split('\n')
  const sepIdx = lines.findIndex((l) => l.length >= 3 && /^=+$/.test(l))

  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []
  const addedHeaders = new Set<string>()

  for (const reass of reassignments) {
    // 1. Replace old_refdes with new_refdes at each position in the body
    for (const [line, startCol, _textWidth] of reass.positions) {
      const oldLen = reass.old_refdes.length
      edits.push({
        range: new monaco.Range(line, startCol, line, startCol + oldLen),
        text: reass.new_refdes,
      })
    }

    // 2. Add header declaration if not already present (in original text or
    //    an already-scheduled edit).
    if (addedHeaders.has(reass.new_refdes)) continue
    const headerLines = sepIdx >= 0 ? lines.slice(0, sepIdx) : []
    const alreadyDeclared = headerLines.some((l) =>
      l.trim().startsWith(reass.new_refdes + ':'),
    )
    if (!alreadyDeclared) {
      addedHeaders.add(reass.new_refdes)
      const declLine = `${reass.new_refdes}: ${reass.symbol_name}`
      if (sepIdx >= 0) {
        edits.push({
          range: new monaco.Range(sepIdx + 1, 1, sepIdx + 1, 1),
          text: declLine + '\n',
        })
      } else {
        edits.push({
          range: new monaco.Range(1, 1, 1, 1),
          text: declLine + '\n==========\n',
        })
      }
    }
  }

  // If the injected component's refdes was auto-incremented, update
  // lastInjectedRefdes so the next sidebar click can find/remove the box.
  if (lastInjectedRefdes) {
    for (const reass of reassignments) {
      if (reass.old_refdes === lastInjectedRefdes) {
        lastInjectedRefdes = reass.new_refdes
      }
    }
  }

  if (edits.length > 0) {
    suppressPreviewClear = true
    editor.executeEdits('refdes-reassign', edits)
  }
}

function doCompile(input: string) {
  try {
    const result = compile(input)
    svgContent.value = result.svg
    kicadContent.value = result.kicad_sch
    // Parse source map once — cursor handler reads the cached array.
    if (result.source_map_json) {
      sourceMapSpans = JSON.parse(result.source_map_json)
    } else {
      sourceMapSpans = []
    }

    // Parse component angles for rotation state tracking.
    if (result.angles_json) {
      try {
        componentAngles = JSON.parse(result.angles_json)
      } catch {
        componentAngles = {}
      }
    }

    // ---- apply refdes reassignments (duplicate instance auto-increment) ----
    applyRefdesReassignments(result.refdes_reassignments_json)

    result.free()
  } catch {
    // ignore parse errors on partial input
    sourceMapSpans = []
  }
}

function isInProtectedSpan(line: number, col: number): ComponentTextSpan | null {
  return (
    sourceMapSpans.find(
      (s) => s.line_number === line && col >= s.start_col && col <= s.end_col,
    ) ?? null
  )
}

function handleGridOverwrite(char: string) {
  if (!editor) return
  const pos = editor.getPosition()!
  const line = pos.lineNumber
  const col = pos.column

  // Entity protection: block overwrite inside a Port/Label span
  if (isInProtectedSpan(line, col)) return

  const model = editor.getModel()!
  const lineLen = model.getLineMaxColumn(line) - 1

  if (col <= lineLen) {
    // Overwrite existing character
    suppressPreviewClear = true
    editor.executeEdits('grid-overwrite', [
      {
        range: new monaco.Range(line, col, line, col + 1),
        text: char,
      },
    ])
    editor.setPosition({ lineNumber: line, column: col + 1 })
  } else {
    // Past end of line — extend line with spaces then append
    const pad = ' '.repeat(col - lineLen - 1)
    suppressPreviewClear = true
    editor.executeEdits('grid-overwrite', [
      {
        range: new monaco.Range(line, lineLen + 1, line, lineLen + 1),
        text: pad + char,
      },
    ])
    editor.setPosition({ lineNumber: line, column: col + 1 })
  }
}

function handleGridDelete(isBackspace: boolean) {
  if (!editor) return
  const pos = editor.getPosition()!
  const line = pos.lineNumber
  const col = pos.column

  // Backspace deletes the character at col-1, not col.  Check the position
  // actually being erased so that Backspace at the left edge of a Port
  // (where col-1 is outside the span) does a normal space-based backspace.
  const checkCol = isBackspace ? col - 1 : col
  const hit = checkCol >= 1 ? isInProtectedSpan(line, checkCol) : null
  if (hit) {
    if (isBackspace || hit.kind === 'Label') {
      // Backspace always, or Delete on a Label: just this single span
      const len = hit.end_col - hit.start_col + 1
      const spaces = ' '.repeat(len)
      suppressPreviewClear = true
      editor.executeEdits('atomic-delete', [
        {
          range: new monaco.Range(
            hit.line_number,
            hit.start_col,
            hit.line_number,
            hit.end_col + 1,
          ),
          text: spaces,
        },
      ])
      editor.setPosition({ lineNumber: hit.line_number, column: hit.start_col })
    } else {
      // Delete on a Port: delete the ENTIRE component (all Port spans with same refdes)
      const siblings = sourceMapSpans.filter(
        (s) => s.refdes === hit.refdes && s.kind === 'Port',
      )
      suppressPreviewClear = true

      const edits: monaco.editor.IIdentifiedSingleEditOperation[] = siblings.map((s) => ({
        range: new monaco.Range(
          s.line_number,
          s.start_col,
          s.line_number,
          s.end_col + 1,
        ),
        text: ' '.repeat(s.end_col - s.start_col + 1),
      }))

      // Also remove the header declaration for this refdes.
      const refdes = hit.refdes
      const lines = editor!.getValue().split('\n')
      for (let i = 0; i < lines.length; i++) {
        if (lines[i].trim().startsWith(refdes + ':')) {
          edits.push({ range: new monaco.Range(i + 1, 1, i + 2, 1), text: '' })
          break
        }
      }
      // If lastInjectedRefdes tracked this component, clear it.
      if (lastInjectedRefdes === refdes) {
        lastInjectedRefdes = null
      }

      editor.executeEdits('atomic-delete-component', edits)

      // Move cursor to a safe position first, then explicitly clear all
      // highlights so that stale decorations are fully removed.
      const first = siblings[0]
      editor.setPosition({ lineNumber: first.line_number, column: first.start_col })

      // Force a synchronous compile + highlight check at the new position.
      doCompile(editor!.getValue())
      const pos = editor.getPosition()!
      matchAndHighlightComponent(pos.lineNumber, pos.column)
    }
    return
  }

  // Normal space-based deletion (no character collapse)
  if (isBackspace) {
    if (col > 1) {
      suppressPreviewClear = true
      editor.executeEdits('grid-delete', [
        {
          range: new monaco.Range(line, col - 1, line, col),
          text: ' ',
        },
      ])
      editor.setPosition({ lineNumber: line, column: col - 1 })
    } else if (line > 1) {
      // Backspace at column 1 — delete entire empty line
      const curLine = editor.getModel()!.getLineContent(line)
      if (curLine.trim() === '') {
        suppressPreviewClear = true
        editor.executeEdits('grid-delete-line', [
          { range: new monaco.Range(line, 1, line + 1, 1), text: '' },
        ])
        const prevLen = editor.getModel()!.getLineMaxColumn(line - 1) - 1
        editor.setPosition({ lineNumber: line - 1, column: prevLen + 1 })
      }
    }
  } else {
    // Delete key
    const lineLen = editor.getModel()!.getLineMaxColumn(line) - 1
    if (col <= lineLen) {
      suppressPreviewClear = true
      editor.executeEdits('grid-delete', [
        {
          range: new monaco.Range(line, col, line, col + 1),
          text: ' ',
        },
      ])
    } else {
      // Delete at end of empty line — remove the whole line
      const curLine = editor.getModel()!.getLineContent(line)
      if (curLine.trim() === '') {
        suppressPreviewClear = true
        editor.executeEdits('grid-delete-line', [
          { range: new monaco.Range(line, 1, line + 1, 1), text: '' },
        ])
        if (line > 1) {
          const prevLen = editor.getModel()!.getLineMaxColumn(line - 1) - 1
          editor.setPosition({ lineNumber: line - 1, column: prevLen + 1 })
        }
      }
    }
  }
}

function handleGridEnter() {
  if (!editor) return
  const pos = editor.getPosition()!
  const line = pos.lineNumber
  const col = pos.column

  // Entity protection: block Enter inside a Port/Label span
  if (isInProtectedSpan(line, col)) return

  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // Only allow Enter when the next line is empty (or we're on the last line).
  // This prevents shifting non-empty grid rows downward and breaking connections.
  if (line < totalLines) {
    const nextLine = model.getLineContent(line + 1)
    if (nextLine.trim() !== '') {
      // Next line has content — just move cursor down (like Down key)
      editor.setPosition({ lineNumber: line + 1, column: 1 })
      return
    }
  }

  const lineContent = model.getLineContent(line)
  const leftPart = lineContent.substring(0, col - 1)
  const rightPart = lineContent.substring(col - 1)

  suppressPreviewClear = true
  // Split the line: replace current line with leftPart\nrightPart
  editor.executeEdits('grid-enter', [
    {
      range: new monaco.Range(line, 1, line, lineContent.length + 1),
      text: leftPart + '\n' + rightPart,
    },
  ])
  editor.setPosition({ lineNumber: line + 1, column: 1 })
}

function handleGridPaste(text: string) {
  if (!editor || !text) return
  const pos = editor.getPosition()!
  if (isInGrid2(pos.lineNumber)) return
  const startLine = pos.lineNumber
  const startCol = pos.column
  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  const pasteLines = text.split('\n')
  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  for (let i = 0; i < pasteLines.length; i++) {
    const content = pasteLines[i]
    const targetLine = startLine + i

    if (targetLine <= totalLines) {
      const curLine = model.getLineContent(targetLine)
      const curLen = curLine.length

      if (startCol <= curLen) {
        // Overwrite within existing line
        const endCol = Math.min(startCol + content.length, curLen + 1)
        const head = content.substring(0, endCol - startCol)
        edits.push({
          range: new monaco.Range(targetLine, startCol, targetLine, endCol),
          text: head,
        })
        // Remaining text extends past end of line — append
        const tail = content.substring(endCol - startCol)
        if (tail) {
          edits.push({
            range: new monaco.Range(targetLine, curLen + 1, targetLine, curLen + 1),
            text: tail,
          })
        }
      } else {
        // Past end of line — pad with spaces then paste
        const pad = ' '.repeat(startCol - curLen - 1)
        edits.push({
          range: new monaco.Range(targetLine, curLen + 1, targetLine, curLen + 1),
          text: pad + content,
        })
      }
    } else {
      // Past end of document — append new lines
      const lastLine = totalLines
      const lastLen = model.getLineMaxColumn(lastLine) - 1
      const pad = ' '.repeat(startCol - 1)
      edits.push({
        range: new monaco.Range(lastLine, lastLen + 1, lastLine, lastLen + 1),
        text: '\n' + pad + content,
      })
    }
  }

  if (edits.length > 0) {
    suppressPreviewClear = true
    editor.executeEdits('grid-paste', edits)
    const lastIdx = pasteLines.length - 1
    const lastCol = startCol + pasteLines[lastIdx].length
    const lastLine = Math.min(startLine + lastIdx, model.getLineCount())
    editor.setPosition({ lineNumber: lastLine, column: lastCol })
    editor.focus()
  }
}

// ================================================================
// Alt+Arrow translation & Alt+R rotation (Step 6.4)
// ================================================================

interface Cell {
  line: number
  col: number
  ch: string
}

/** Check whether moving `oldCells` to `newCells` would cross a rigid `====` separator. */
function wouldCrossSeparator(oldCells: Cell[], newCells: Cell[]): boolean {
  if (!editor) return false
  const seps = findSeparators(editor.getValue())
  const sepLines = seps.map(s => s + 1) // 0-based → 1-based Monaco

  for (const sepLine of sepLines) {
    for (let i = 0; i < newCells.length; i++) {
      const oldSide = oldCells[i].line < sepLine ? -1 : oldCells[i].line > sepLine ? 1 : 0
      const newSide = newCells[i].line < sepLine ? -1 : newCells[i].line > sepLine ? 1 : 0
      if (newSide === 0) return true // landed directly on separator
      if (oldSide !== 0 && newSide !== oldSide) return true // crossed separator
    }
  }
  return false
}

function handleAltShortcut(e: KeyboardEvent): boolean {
  if (e.key === 'ArrowUp')    { handleTranslate(-1, 0); return true }
  if (e.key === 'ArrowDown')  { handleTranslate(1, 0); return true }
  if (e.key === 'ArrowLeft')  { handleTranslate(0, -1); return true }
  if (e.key === 'ArrowRight') { handleTranslate(0, 1); return true }
  if (e.key === 'r' || e.key === 'R') { rotateComponent(); return true }
  return false
}

function handleTranslate(dL: number, dC: number) {
  if (!editor) return
  const pos = editor.getPosition()!

  // Group move: when a box selection is active, move all selected
  // components regardless of where the cursor sits in the rectangle.
  if (activeRefDesList.value.length > 0) {
    // Block group move if the selection is entirely within Grid2.
    if (currentBoxSelection && isInGrid2(currentBoxSelection.minLine)) return
    translateGroup(dL, dC, pos)
    return
  }

  // Single-component move: blocked in Grid2.
  if (isInGrid2(pos.lineNumber)) return
  const hit = isInProtectedSpan(pos.lineNumber, pos.column)
  if (!hit) return
  translateComponent(dL, dC)
}

// ---- Group Move Solver (Step 6.5) ---------------------------------------
// Collects all cells from every component in activeRefDesList, performs
// collective collision detection against non-selected components, and
// rewrites all old/new footprints in a single undo-able transaction.
function translateGroup(dL: number, dC: number, _pos: { lineNumber: number; column: number }) {
  if (!editor) return
  const selectedRefdes = new Set(activeRefDesList.value)
  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // 1. Gather ALL characters within the selection rectangle — not just
  //    component footprints, but also wires, junctions, spaces, etc.
  //    This makes the move a true "block move".
  const oldCells: Cell[] = []
  if (currentBoxSelection) {
    const { minLine, maxLine, minCol, maxCol } = currentBoxSelection
    for (let line = minLine; line <= maxLine; line++) {
      if (line < 1 || line > totalLines) continue
      const lineContent = model.getLineContent(line)
      for (let col = minCol; col <= maxCol; col++) {
        oldCells.push({
          line,
          col,
          ch: lineContent[col - 1] || ' ',
        })
      }
    }
  }
  if (oldCells.length === 0) return

  // 2. Compute new positions.
  const newCells: Cell[] = oldCells.map(c => ({
    line: c.line + dL,
    col: c.col + dC,
    ch: c.ch,
  }))

  // 3. Boundary check (edges + separator lines).
  for (const nc of newCells) {
    if (nc.line < 1 || nc.col < 1) return
  }
  // Reject moves that would cross a separator line (rigid boundary).
  if (wouldCrossSeparator(oldCells, newCells)) return

  // 4. Collision detection — reject if any new cell overlaps a
  //    source-map span belonging to a NON-selected component.
  for (const nc of newCells) {
    const collision = sourceMapSpans.find(
      s =>
        !selectedRefdes.has(s.refdes) &&
        s.line_number === nc.line &&
        nc.col >= s.start_col &&
        nc.col <= s.end_col,
    )
    if (collision) return
  }

  // 5. Build per-line edits (erase old, write new in one pass).
  interface LineOp { col: number; ch: string }
  const lineOps = new Map<number, LineOp[]>()

  for (const c of oldCells) {
    if (!lineOps.has(c.line)) lineOps.set(c.line, [])
    lineOps.get(c.line)!.push({ col: c.col, ch: ' ' })
  }
  for (const nc of newCells) {
    if (!lineOps.has(nc.line)) lineOps.set(nc.line, [])
    lineOps.get(nc.line)!.push({ col: nc.col, ch: nc.ch })
  }

  const existingLines: number[] = []
  const beyondLines: number[] = []
  for (const line of lineOps.keys()) {
    if (line <= totalLines) existingLines.push(line)
    else beyondLines.push(line)
  }

  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  for (const line of existingLines) {
    const ops = lineOps.get(line)!
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    let newLen = oldLen
    for (const op of ops) {
      if (op.col > newLen) newLen = op.col
    }
    const arr = [...oldContent.padEnd(newLen, ' ')]
    for (const op of ops) {
      arr[op.col - 1] = op.ch
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({
        range: new monaco.Range(line, 1, line, oldLen + 1),
        text: newContent,
      })
    }
  }

  if (beyondLines.length > 0) {
    beyondLines.sort((a, b) => a - b)
    const parts: string[] = []
    let prevLine = totalLines
    for (const line of beyondLines) {
      for (let l = prevLine + 1; l < line; l++) {
        parts.push('')
      }
      const ops = lineOps.get(line)!
      const maxCol = Math.max(...ops.map(o => o.col))
      const arr = new Array<string>(maxCol).fill(' ')
      for (const op of ops) {
        arr[op.col - 1] = op.ch
      }
      parts.push(arr.join(''))
      prevLine = line
    }
    const lastLine = totalLines
    const lastLineLen = model.getLineMaxColumn(lastLine)
    edits.push({
      range: new monaco.Range(lastLine, lastLineLen, lastLine, lastLineLen),
      text: '\n' + parts.join('\n'),
    })
  }

  if (edits.length === 0) return

  suppressPreviewClear = true
  editor.executeEdits('group-move', edits)

  // 6. Shift the selection rectangle and restore as proper column
  //    selection (one Selection per line) so the visual box follows.
  if (currentBoxSelection) {
    const shifted = {
      minLine: currentBoxSelection.minLine + dL,
      maxLine: currentBoxSelection.maxLine + dL,
      minCol: currentBoxSelection.minCol + dC,
      maxCol: currentBoxSelection.maxCol + dC,
    }
    const selections: monaco.Selection[] = []
    for (let ln = shifted.minLine; ln <= shifted.maxLine; ln++) {
      selections.push(
        new monaco.Selection(ln, shifted.minCol, ln, shifted.maxCol),
      )
    }
    editor.setSelections(selections)
  }

  // 8. Recompile — the box selection will be re-scanned automatically
  //    when onDidChangeCursorSelection fires from setSelection above.
  doCompile(editor.getValue())
}

function translateComponent(dL: number, dC: number) {
  if (!editor) return
  const pos = editor.getPosition()!
  const hit = isInProtectedSpan(pos.lineNumber, pos.column)
  if (!hit) return

  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // Gather cells and decide collision-exclusion predicate.
  let cells: Cell[]
  let excludeSelf: (s: ComponentTextSpan) => boolean

  if (hit.kind === 'Port') {
    // Component: all port spans sharing the same refdes.
    const spans = sourceMapSpans.filter(s => s.refdes === hit.refdes && s.kind === 'Port')
    cells = []
    for (const span of spans) {
      const lineContent = model.getLineContent(span.line_number)
      for (let c = span.start_col; c <= span.end_col; c++) {
        cells.push({
          line: span.line_number,
          col: c,
          ch: lineContent[c - 1] || ' ',
        })
      }
    }
    excludeSelf = (s) => s.refdes === hit.refdes
  } else if (hit.kind === 'Label') {
    // Label: just the single span under the cursor.
    const lineContent = model.getLineContent(hit.line_number)
    cells = []
    for (let c = hit.start_col; c <= hit.end_col; c++) {
      cells.push({
        line: hit.line_number,
        col: c,
        ch: lineContent[c - 1] || ' ',
      })
    }
    excludeSelf = (s) =>
      s.line_number === hit.line_number &&
      s.start_col === hit.start_col &&
      s.end_col === hit.end_col
  } else {
    return
  }

  // 2. Compute new positions.
  const newCells: Cell[] = cells.map(c => ({
    line: c.line + dL,
    col: c.col + dC,
    ch: c.ch,
  }))

  // 3. Boundary check — never allow col < 1 or line < 1.
  for (const nc of newCells) {
    if (nc.line < 1 || nc.col < 1) return
  }
  // Reject moves that would cross a separator line (rigid boundary).
  if (wouldCrossSeparator(cells, newCells)) return

  // 4. Collision detection — reject if any new cell overlaps another
  //    entity's source-map span (excluding our own cells).
  for (const nc of newCells) {
    const collision = sourceMapSpans.find(s =>
      !excludeSelf(s) &&
      s.line_number === nc.line &&
      nc.col >= s.start_col &&
      nc.col <= s.end_col,
    )
    if (collision) return
  }

  // 5. Build per-line edits (erase old cells, write new cells).
  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  // Group erase + write ops by line.
  interface LineOp { col: number; ch: string }
  const lineOps = new Map<number, LineOp[]>()

  for (const c of cells) {
    if (!lineOps.has(c.line)) lineOps.set(c.line, [])
    lineOps.get(c.line)!.push({ col: c.col, ch: ' ' })
  }
  for (const nc of newCells) {
    if (!lineOps.has(nc.line)) lineOps.set(nc.line, [])
    lineOps.get(nc.line)!.push({ col: nc.col, ch: nc.ch })
  }

  // Split: lines already in the document vs lines that need to be appended.
  const existingLines: number[] = []
  const beyondLines: number[] = []
  for (const line of lineOps.keys()) {
    if (line <= totalLines) existingLines.push(line)
    else beyondLines.push(line)
  }

  for (const line of existingLines) {
    const ops = lineOps.get(line)!
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    let newLen = oldLen
    for (const op of ops) {
      if (op.col > newLen) newLen = op.col
    }
    const arr = [...oldContent.padEnd(newLen, ' ')]
    for (const op of ops) {
      arr[op.col - 1] = op.ch
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({
        range: new monaco.Range(line, 1, line, oldLen + 1),
        text: newContent,
      })
    }
  }

  // Append new lines (with cells placed) in one edit at end of document.
  if (beyondLines.length > 0) {
    beyondLines.sort((a, b) => a - b)
    const parts: string[] = []
    let prevLine = totalLines
    for (const line of beyondLines) {
      // Pad with empty lines if there are gaps.
      for (let l = prevLine + 1; l < line; l++) {
        parts.push('')
      }
      const ops = lineOps.get(line)!
      const maxCol = Math.max(...ops.map(o => o.col))
      const arr = new Array<string>(maxCol).fill(' ')
      for (const op of ops) {
        arr[op.col - 1] = op.ch
      }
      parts.push(arr.join(''))
      prevLine = line
    }
    const lastLine = totalLines
    const lastLineLen = model.getLineMaxColumn(lastLine)
    edits.push({
      range: new monaco.Range(lastLine, lastLineLen, lastLine, lastLineLen),
      text: '\n' + parts.join('\n'),
    })
  }

  if (edits.length === 0) return

  suppressPreviewClear = true
  editor.executeEdits('component-move', edits)

  // 6. Cursor follows the move — keep relative position within the component.
  editor.setPosition({
    lineNumber: pos.lineNumber + dL,
    column: pos.column + dC,
  })

  // 7. Recompile and re-highlight.
  doCompile(editor.getValue())
  const newPos = editor.getPosition()!
  matchAndHighlightComponent(newPos.lineNumber, newPos.column)
}

function getSymbolName(refdes: string): string | null {
  if (!editor) return null
  const text = editor.getValue()
  const lines = text.split('\n')
  const sepIdx = lines.findIndex(l => /^=+$/.test(l))
  const headerEnd = sepIdx >= 0 ? sepIdx : lines.length
  for (let i = 0; i < headerEnd; i++) {
    const m = lines[i].match(/^\s*(\S+)\s*:\s*(\S+)/)
    if (m && m[1] === refdes) return m[2]
  }
  return null
}

function rotateComponent() {
  if (!editor) return
  const pos = editor.getPosition()!
  // Rotation only allowed in Grid1.
  if (isInGrid2(pos.lineNumber)) return
  const hit = isInProtectedSpan(pos.lineNumber, pos.column)
  if (!hit || hit.kind !== 'Port') return

  const refdes = hit.refdes
  const symbolName = getSymbolName(refdes)
  if (!symbolName) return

  const currentAngle = componentAngles[refdes] ?? 0
  const newAngle = (currentAngle + 90) % 360

  // Get the new footprint from WASM.
  const json = get_rotated_footprint(symbolName, refdes, newAngle)
  let entries: { dr: number; dc: number; ch: string }[]
  try {
    entries = JSON.parse(json)
  } catch {
    return
  }
  if (!entries || entries.length === 0) return

  // Anchor = refdes text span (length > 1; arrows are always 1 char wide).
  const allPortSpans = sourceMapSpans.filter(s => s.refdes === refdes && s.kind === 'Port')
  const anchorSpan = allPortSpans.find(s => s.end_col - s.start_col + 1 >= refdes.length)
    ?? allPortSpans[0]
  if (!anchorSpan) return
  const anchorLine = anchorSpan.line_number
  const anchorCol = anchorSpan.start_col

  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // Compute new absolute cells.
  const newCells: Cell[] = entries.map(e => ({
    line: anchorLine + e.dr,
    col: anchorCol + e.dc,
    ch: e.ch,
  }))

  // Boundary check.
  for (const nc of newCells) {
    if (nc.line < 1 || nc.col < 1) return
  }

  // Collision check — exclude our own refdes.
  for (const nc of newCells) {
    const collision = sourceMapSpans.find(s =>
      s.refdes !== refdes &&
      s.line_number === nc.line &&
      nc.col >= s.start_col &&
      nc.col <= s.end_col,
    )
    if (collision) return
  }

  // Gather old cells (from source map).
  const oldSpans = sourceMapSpans.filter(s => s.refdes === refdes && s.kind === 'Port')
  const oldCells: Cell[] = []
  for (const span of oldSpans) {
    const lineContent = model.getLineContent(span.line_number)
    for (let c = span.start_col; c <= span.end_col; c++) {
      oldCells.push({
        line: span.line_number,
        col: c,
        ch: lineContent[c - 1] || ' ',
      })
    }
  }

  // Build per-line edits (erase old, write new).
  interface LineOp { col: number; ch: string }
  const lineOps = new Map<number, LineOp[]>()

  for (const c of oldCells) {
    if (!lineOps.has(c.line)) lineOps.set(c.line, [])
    lineOps.get(c.line)!.push({ col: c.col, ch: ' ' })
  }
  for (const nc of newCells) {
    if (!lineOps.has(nc.line)) lineOps.set(nc.line, [])
    lineOps.get(nc.line)!.push({ col: nc.col, ch: nc.ch })
  }

  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  // Split: lines already in the document vs lines that need to be appended.
  const existingLinesR: number[] = []
  const beyondLinesR: number[] = []
  for (const line of lineOps.keys()) {
    if (line <= totalLines) existingLinesR.push(line)
    else beyondLinesR.push(line)
  }

  for (const line of existingLinesR) {
    const ops = lineOps.get(line)!
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    let newLen = oldLen
    for (const op of ops) {
      if (op.col > newLen) newLen = op.col
    }
    const arr = [...oldContent.padEnd(newLen, ' ')]
    for (const op of ops) {
      arr[op.col - 1] = op.ch
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({
        range: new monaco.Range(line, 1, line, oldLen + 1),
        text: newContent,
      })
    }
  }

  // Append new lines (with cells placed) in one edit at end of document.
  if (beyondLinesR.length > 0) {
    beyondLinesR.sort((a, b) => a - b)
    const parts: string[] = []
    let prevLine = totalLines
    for (const line of beyondLinesR) {
      for (let l = prevLine + 1; l < line; l++) {
        parts.push('')
      }
      const ops = lineOps.get(line)!
      const maxCol = Math.max(...ops.map(o => o.col))
      const arr = new Array<string>(maxCol).fill(' ')
      for (const op of ops) {
        arr[op.col - 1] = op.ch
      }
      parts.push(arr.join(''))
      prevLine = line
    }
    const lastLine = totalLines
    const lastLineLen = model.getLineMaxColumn(lastLine)
    edits.push({
      range: new monaco.Range(lastLine, lastLineLen, lastLine, lastLineLen),
      text: '\n' + parts.join('\n'),
    })
  }

  if (edits.length === 0) return

  suppressPreviewClear = true
  editor.executeEdits('component-rotate', edits)

  // Keep cursor on the same relative position (or anchor start if old position is gone).
  const relCol = pos.column - anchorCol
  const relLine = pos.lineNumber - anchorLine
  const wasGroup = activeRefDesList.value.length > 1
  const savedBox = currentBoxSelection

  editor.setPosition({
    lineNumber: anchorLine + relLine,
    column: Math.max(anchorCol, anchorCol + relCol),
  })

  doCompile(editor.getValue())

  if (wasGroup && savedBox) {
    // Re-establish box selection so the rotated component stays highlighted
    // if it still falls within the original rectangle.
    editor.setSelection(
      new monaco.Selection(
        savedBox.minLine, savedBox.minCol,
        savedBox.maxLine, savedBox.maxCol,
      ),
    )
  } else {
    const newPos = editor.getPosition()!
    matchAndHighlightComponent(newPos.lineNumber, newPos.column)
  }
}

function downloadKicad() {
  const blob = new Blob([kicadContent.value], {
    type: 'text/plain;charset=utf-8',
  })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'schematic.kicad_sch'
  a.click()
  URL.revokeObjectURL(url)
}

/** Build edits that erase all cells of `refdes`'s Port spans (set to spaces). */
function eraseFootprintEdits(refdes: string): monaco.editor.IIdentifiedSingleEditOperation[] {
  if (!editor) return []
  const spans = sourceMapSpans.filter(s => s.refdes === refdes && s.kind === 'Port')
  if (spans.length === 0) return []

  const model = editor.getModel()!
  interface LineOp { col: number; ch: string }
  const lineOps = new Map<number, LineOp[]>()

  for (const span of spans) {
    for (let c = span.start_col; c <= span.end_col; c++) {
      if (!lineOps.has(span.line_number)) lineOps.set(span.line_number, [])
      lineOps.get(span.line_number)!.push({ col: c, ch: ' ' })
    }
  }

  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []
  for (const [line, ops] of lineOps) {
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    let newLen = oldLen
    for (const op of ops) {
      if (op.col > newLen) newLen = op.col
    }
    const arr = [...oldContent.padEnd(newLen, ' ')]
    for (const op of ops) {
      arr[op.col - 1] = op.ch
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({ range: new monaco.Range(line, 1, line, oldLen + 1), text: newContent })
    }
  }
  return edits
}

function removeInjectedComponent() {
  if (!editor || !lastInjectedRefdes) return

  const oldRefdes = lastInjectedRefdes
  lastInjectedRefdes = null

  const lines = editor.getValue().split('\n')
  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  // Remove header declaration.
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim().startsWith(oldRefdes + ':')) {
      edits.push({ range: new monaco.Range(i + 1, 1, i + 2, 1), text: '' })
      break
    }
  }

  // Erase footprint cells in-place.
  edits.push(...eraseFootprintEdits(oldRefdes))

  if (edits.length > 0) {
    suppressPreviewClear = true
    editor.executeEdits('clear-preview', edits)
  }
}

function injectComponent(symbol: string, prefix: string) {
  if (!editor) return

  const fullText = editor.getValue()
  const lines = fullText.split('\n')
  const edits: monaco.editor.IIdentifiedSingleEditOperation[] = []

  // ---- Step 0: remove previous preview zone ----------------------------
  const oldRefdes = lastInjectedRefdes
  lastInjectedRefdes = null

  // Record old stub's start line before erasing.
  let oldStubLine = 0
  if (oldRefdes) {
    const oldSpans = sourceMapSpans.filter(s => s.refdes === oldRefdes && s.kind === 'Port')
    if (oldSpans.length > 0) {
      oldStubLine = Math.min(...oldSpans.map(s => s.line_number))
    }

    // Remove header declaration.
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trim().startsWith(oldRefdes + ':')) {
        edits.push({ range: new monaco.Range(i + 1, 1, i + 2, 1), text: '' })
        break
      }
    }
  }

  // ---- Step 1: find separators -----------------------------------------
  const seps = findSeparators(fullText)
  const sep1Idx = seps[0] ?? -1
  const sep2Idx = seps[1] ?? -1

  // ---- Step 2: stub always placed at leftmost column -------------------

  // ---- Step 3: pick next unused refdes number --------------------------
  const headerLines = sep1Idx >= 0 ? lines.slice(0, sep1Idx) : []
  const existingNums: number[] = []
  const escapedPrefix = prefix.replace(/[#]/g, '\\#')
  const refdesRegex = new RegExp(`^\\s*${escapedPrefix}(\\d+)\\s*:`)
  for (const line of headerLines) {
    if (oldRefdes && line.trim().startsWith(oldRefdes + ':')) continue
    const m = line.match(refdesRegex)
    if (m) existingNums.push(parseInt(m[1]))
  }
  const nextNum = existingNums.length > 0 ? Math.max(...existingNums) + 1 : 1
  const refdes = prefix + String(nextNum)
  const filledStub = generate_stub(symbol, refdes)
  const headerLine = `${refdes}: ${symbol}`

  // ---- Step 4: insert declaration above sep1 ---------------------------
  const alreadyDeclared = headerLines.some(
    (l) =>
      !(oldRefdes && l.trim().startsWith(oldRefdes + ':')) &&
      l.trim().startsWith(refdes + ':'),
  )
  if (!alreadyDeclared) {
    if (sep1Idx >= 0) {
      edits.push({
        range: new monaco.Range(sep1Idx + 1, 1, sep1Idx + 1, 1),
        text: headerLine + '\n',
      })
    } else {
      edits.push({
        range: new monaco.Range(1, 1, 1, 1),
        text: headerLine + '\n==========\n',
      })
    }
  }

  // ---- Step 5: write stub cells at the old position (or default in Grid2) -
  const stubLines = filledStub.split('\n')
  // Determine target start line (1-based Monaco).
  const targetLine = oldStubLine > 0
    ? oldStubLine
    : sep2Idx >= 0
      ? sep2Idx + 3  // first injection: 2 lines below sep2
      : lines.length + 2  // no Grid2 yet — will append

  const model = editor.getModel()!
  const totalLines = model.getLineCount()

  // Build per-line operations: erase old cells + write new cells.
  interface LineOp { col: number; ch: string }
  const lineOps = new Map<number, LineOp[]>()

  // Erase old footprint cells.
  if (oldRefdes) {
    const oldSpans = sourceMapSpans.filter(s => s.refdes === oldRefdes && s.kind === 'Port')
    for (const span of oldSpans) {
      if (!lineOps.has(span.line_number)) lineOps.set(span.line_number, [])
      for (let c = span.start_col; c <= span.end_col; c++) {
        lineOps.get(span.line_number)!.push({ col: c, ch: ' ' })
      }
    }
  }

  // Write new stub cells.
  for (let r = 0; r < stubLines.length; r++) {
    const line = targetLine + r
    const content = stubLines[r]
    for (let c = 0; c < content.length; c++) {
      if (content[c] === ' ') continue
      if (!lineOps.has(line)) lineOps.set(line, [])
      lineOps.get(line)!.push({ col: c + 1, ch: content[c] })
    }
  }

  // Generate per-line edits for existing lines.
  for (const [line, ops] of lineOps) {
    if (line < 1 || line > totalLines) continue
    const oldContent = model.getLineContent(line)
    const oldLen = oldContent.length
    let newLen = oldLen
    for (const op of ops) {
      if (op.col > newLen) newLen = op.col
    }
    const arr = [...oldContent.padEnd(newLen, ' ')]
    for (const op of ops) {
      arr[op.col - 1] = op.ch
    }
    const newContent = arr.join('')
    if (newContent !== oldContent) {
      edits.push({
        range: new monaco.Range(line, 1, line, oldLen + 1),
        text: newContent,
      })
    }
  }

  // Append new lines for cells beyond the current document.
  const beyondLines = [...lineOps.keys()].filter(l => l > totalLines).sort((a, b) => a - b)
  if (beyondLines.length > 0) {
    const parts: string[] = []
    let prevLine = totalLines
    for (const line of beyondLines) {
      for (let l = prevLine + 1; l < line; l++) {
        parts.push('')
      }
      const ops = lineOps.get(line)!
      const maxCol = Math.max(...ops.map(o => o.col))
      const arr = new Array<string>(maxCol).fill(' ')
      for (const op of ops) {
        arr[op.col - 1] = op.ch
      }
      parts.push(arr.join(''))
      prevLine = line
    }
    // If no Grid2 exists yet, prepend separator + blank lines.
    if (sep2Idx < 0) {
      parts.unshift('', '='.repeat(45), '')
    }
    const lastLine = totalLines
    const lastLineLen = model.getLineMaxColumn(lastLine)
    edits.push({
      range: new monaco.Range(lastLine, lastLineLen, lastLine, lastLineLen),
      text: '\n' + parts.join('\n'),
    })
  }

  suppressPreviewClear = true
  editor.executeEdits('component-inject', edits)
  lastInjectedRefdes = refdes
  editor.focus()
}
</script>

<template>
  <div class="app-shell">
    <!-- Loading overlay -->
    <div v-if="!wasmReady" class="loading">
      <span>Loading WASM engine...</span>
    </div>

    <template v-else>
      <!-- Component Library Sidebar -->
      <div class="pane pane-sidebar">
        <div class="pane-header">Components</div>
        <div class="sidebar-list">
          <button
            v-for="comp in COMPONENT_LIBRARY"
            :key="comp.symbol"
            class="sidebar-item"
            @click="injectComponent(comp.symbol, comp.prefix)"
          >
            <span class="sidebar-symbol">{{ comp.symbol }}</span>
            <span class="sidebar-label">{{ comp.label }}</span>
          </button>
        </div>
      </div>

      <!-- Left: Monaco Editor -->
      <div class="pane pane-left">
        <div class="pane-header">ASCII Schematic</div>
        <div ref="editorContainer" class="editor-container"></div>
      </div>

      <!-- Right: Result tabs -->
      <div class="pane pane-right">
        <div class="tabs">
          <button
            :class="['tab', { active: activeTab === 'svg' }]"
            @click="activeTab = 'svg'"
          >
            SVG Preview
          </button>
          <button
            :class="['tab', { active: activeTab === 'kicad' }]"
            @click="activeTab = 'kicad'"
          >
            KiCad Source
          </button>
          <button
            v-if="activeTab === 'kicad' && kicadContent"
            class="tab tab-action"
            @click="downloadKicad"
          >
            Download .kicad_sch
          </button>
        </div>

        <!-- SVG panel -->
        <div v-show="activeTab === 'svg'" class="panel svg-panel">
          <div v-if="!svgContent" class="panel-empty">No output</div>
          <div
            v-else
            v-html="svgContent"
            :class="['svg-wrapper', { highlighted: activeRefDes }]"
          ></div>
        </div>

        <!-- KiCad panel -->
        <div v-show="activeTab === 'kicad'" class="panel kicad-panel">
          <pre v-if="kicadContent" class="kicad-code">{{ kicadContent }}</pre>
          <div v-else class="panel-empty">No output</div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

/* ---- Loading ---- */
.loading {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
  color: var(--text-muted);
  font-size: 18px;
  z-index: 10;
}

/* ---- Panes ---- */
.pane {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.pane-sidebar {
  flex: 0 0 180px;
  border-right: 1px solid var(--border);
  min-width: 0;
  overflow: hidden;
}

.pane-left {
  flex: 1 1 50%;
  border-right: 1px solid var(--border);
  min-width: 0;
}

.pane-right {
  flex: 1 1 50%;
  min-width: 0;
}

.pane-header {
  flex-shrink: 0;
  padding: 8px 16px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: var(--text-muted);
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
}

.editor-container {
  flex: 1;
  min-height: 0;
}

/* ---- Sidebar ---- */
.sidebar-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.sidebar-item {
  display: flex;
  flex-direction: column;
  width: 100%;
  padding: 8px 16px;
  border: none;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: 13px;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s;
}

.sidebar-item:hover {
  background: var(--bg-panel);
}

.sidebar-symbol {
  font-weight: 700;
  font-family: var(--mono);
  font-size: 13px;
  color: #8B0000;
}

.sidebar-label {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 1px;
}

/* ---- Tabs ---- */
.tabs {
  display: flex;
  flex-shrink: 0;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
}

.tab {
  padding: 8px 16px;
  font-size: 13px;
  color: var(--text-muted);
  background: var(--bg-tab);
  border: none;
  border-right: 1px solid var(--border);
  cursor: pointer;
  font-family: inherit;
  transition: background 0.15s, color 0.15s;
}

.tab:hover {
  background: var(--bg);
  color: var(--text);
}

.tab.active {
  background: var(--bg);
  color: var(--text);
  border-bottom: 2px solid var(--accent);
  margin-bottom: -1px;
}

.tab-action {
  margin-left: auto;
  border-right: none;
  border-left: 1px solid var(--border);
  color: var(--accent);
}

.tab-action:hover {
  color: #fff;
  background: var(--accent-bg);
}

/* ---- Panels ---- */
.panel {
  flex: 1;
  overflow: auto;
  min-height: 0;
}

.panel-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-muted);
  font-size: 14px;
}

/* ---- SVG ---- */
.svg-panel {
  background: #ffffff;
}

.svg-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  padding: 24px;
}

.svg-wrapper :deep(svg) {
  max-width: 100%;
  max-height: 100%;
  height: auto;
}

/* ---- KiCad ---- */
.kicad-panel {
  background: var(--bg);
}

.kicad-code {
  padding: 16px;
  font-family: var(--mono);
  font-size: 13px;
  line-height: 1.6;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-all;
  tab-size: 2;
  margin: 0;
}

/* ---- SVG component highlighting ------------------------------------ */
/* Base transition for all component groups (pierces v-html shadow DOM). */
.svg-wrapper :deep([data-refdes]) {
  transition: stroke 0.2s, filter 0.2s;
}

/* The active highlight rule is injected dynamically via JS
   (see setSvgHighlightRule) so it scales to any refdes. */
</style>

<!-- Non-scoped: Monaco editor decorations live outside Vue's shadow DOM. -->
<style>
.monaco-component-active {
  background-color: rgba(66, 133, 244, 0.1);
  border-bottom: 2px solid rgba(66, 133, 244, 0.55);
  border-radius: 2px;
}

.monaco-component-selected {
  background-color: rgba(255, 193, 7, 0.2);
  border-bottom: 2px solid rgba(255, 193, 7, 0.6);
  border-radius: 2px;
}
</style>
