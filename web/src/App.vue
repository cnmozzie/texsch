<script setup lang="ts">
import { ref, onMounted, nextTick } from 'vue'
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import init, { compile } from './wasm/texsch'

self.MonacoEnvironment = {
  getWorker() {
    return new editorWorker()
  },
}

const DEFAULT_INPUT = `U1: OPA330xxD
R1: R
R2: R
R3: R
J1: Conn_01x03_Socket
J2: Conn_Coaxial
J3: Conn_Coaxial
J4: Conn_Coaxial
#GND1: GND
#GND2: GND
#GND3: GND
#GND4: GND
#VCC1: VCC
#VSS1: VSS
=============================================
[In1]--J3:1<                                                       #VCC1:1v
        J3:2v                                                      |
        |                                                          U1:7(V+)^
        #GND3:1^                  +------------------------------U1:3(+)<
                                  |                                       U1:6>--------*--------+--[OUT]
                                  #GND1:1^             +---------U1:2(-)<              |        |
[In2]--J4:1<                                           |           U1:4(V-)v           |        |
        J4:2v                                          |           |                   |        +--J2:1<
        |                                              |           #VSS1:1^            |            J2:2v
        #GND4:1^            [In1]--R1:1< R1:2>---------*                               |            |
                                                       |                               |            #GND2:1^
[VCC]--J1:1<                [In2]--R2:1< R2:2>---------*                               |
[GND]--J1:2<                                           |                               |
[VSS]--J1:3<                                           +------------------R3:1< R3:2>--*`

const svgContent = ref('')
const kicadContent = ref('')
const activeTab = ref<'svg' | 'kicad'>('svg')
const wasmReady = ref(false)
const editorContainer = ref<HTMLElement | null>(null)

let editor: monaco.editor.IStandaloneCodeEditor | null = null

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
    wordWrap: 'on',
  })

  editor.onDidChangeModelContent(() => {
    const text = editor!.getValue()
    doCompile(text)
  })

  doCompile(DEFAULT_INPUT)
})

function doCompile(input: string) {
  try {
    const result = compile(input)
    svgContent.value = result.svg
    kicadContent.value = result.kicad_sch
    result.free()
  } catch {
    // ignore parse errors on partial input
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
</script>

<template>
  <div class="app-shell">
    <!-- Loading overlay -->
    <div v-if="!wasmReady" class="loading">
      <span>Loading WASM engine...</span>
    </div>

    <template v-else>
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
          <div v-else v-html="svgContent" class="svg-wrapper"></div>
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
</style>
