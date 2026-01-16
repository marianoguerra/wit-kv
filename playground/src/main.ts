import './style.css';
import { witKvService, type KeyspaceInfo } from './services/wit-kv-service';
import {
  witAstService,
  type BinaryExport,
  type ValueTree,
} from './services/wit-ast-service';
import { formatHexdump, formatByteCount } from './utils/hexdump';
import { renderValueTree, valueTreeToJson } from './utils/tree-renderer';
import { exampleCategories } from './examples';

/**
 * Decode a binary-export envelope from canonical ABI encoded bytes.
 *
 * The server sends a canonical ABI encoded `binary-export` record:
 * - First 20 bytes: flat buffer containing pointers
 *   - bytes 0-3: ptr for value (little-endian u32)
 *   - bytes 4-7: len for value (little-endian u32)
 *   - byte 8: discriminant for memory option (0=none, 1=some)
 *   - bytes 9-11: padding
 *   - bytes 12-15: ptr for memory (if some)
 *   - bytes 16-19: len for memory (if some)
 * - Remaining bytes: linear memory containing actual list data
 */
function decodeBinaryExportEnvelope(data: ArrayBuffer): BinaryExport {
  const FLAT_SIZE = 20;

  if (data.byteLength < FLAT_SIZE) {
    throw new Error(`Binary export too small: ${data.byteLength} bytes, need at least ${FLAT_SIZE}`);
  }

  const view = new DataView(data);

  // Read value pointer and length (little-endian)
  const valuePtr = view.getUint32(0, true);
  const valueLen = view.getUint32(4, true);

  // Read memory option discriminant
  const memoryDiscriminant = view.getUint8(8);

  // Linear memory starts after the flat buffer
  const linearMemory = new Uint8Array(data, FLAT_SIZE);

  // Extract value bytes from linear memory
  if (valuePtr + valueLen > linearMemory.length) {
    throw new Error(`Invalid value pointer: ${valuePtr}+${valueLen} exceeds memory size ${linearMemory.length}`);
  }
  const value = new Uint8Array(linearMemory.buffer, linearMemory.byteOffset + valuePtr, valueLen);

  // Extract memory bytes if present
  let memory: Uint8Array | undefined;
  if (memoryDiscriminant === 1) {
    const memoryPtr = view.getUint32(12, true);
    const memoryLen = view.getUint32(16, true);
    if (memoryPtr + memoryLen > linearMemory.length) {
      throw new Error(`Invalid memory pointer: ${memoryPtr}+${memoryLen} exceeds memory size ${linearMemory.length}`);
    }
    memory = new Uint8Array(linearMemory.buffer, linearMemory.byteOffset + memoryPtr, memoryLen);
  }

  return { value, memory };
}

// State
let connected = false;
let databases: string[] = [];
let keyspaces: KeyspaceInfo[] = [];
let keys: string[] = [];

let currentDatabase = 'default';
let currentKeyspace: string | null = null;
let currentKey: string | null = null;

let currentBinaryTab: 'hexdump' | 'tree' | 'wave' | 'json' = 'hexdump';

// Binary data cache for current value
let currentBinaryData: ArrayBuffer | null = null;
let currentValueTree: ValueTree | null = null;
let currentWaveText: string | null = null;

// Initialize the app
async function init() {
  renderApp();
  bindEvents();

  // Try to connect automatically
  try {
    await connect();
  } catch {
    // Connection will be retried when user interacts
  }

  // Load wit-ast in background
  witAstService.load().catch(console.error);
}

function renderApp() {
  const app = document.getElementById('app');
  if (!app) return;

  app.innerHTML = `
    <header class="header">
      <h1>wit-kv Playground</h1>
      <div class="header-actions">
        <div class="examples-dropdown">
          <button class="secondary" id="examplesBtn">Load Example</button>
          <div class="examples-menu hidden" id="examplesMenu">
            ${exampleCategories
              .map(
                (cat) => `
              <div class="example-category">${cat.name}</div>
              ${cat.examples
                .map(
                  (ex) => `
                <div class="example-item" data-keyspace="${ex.keyspace}">
                  <div class="title">${ex.name}</div>
                  <div class="desc">${ex.description}</div>
                </div>
              `
                )
                .join('')}
            `
              )
              .join('')}
          </div>
        </div>
        <select id="databaseSelect">
          <option value="default">default</option>
        </select>
        <span id="connectionStatus" class="status disconnected">
          <span class="status-dot"></span>
          Disconnected
        </span>
      </div>
    </header>

    <div class="main-layout">
      <div class="sidebar">
        <div class="sidebar-section">
          <div class="sidebar-header">
            Keyspaces
            <button class="secondary small" id="refreshKeyspacesBtn">Refresh</button>
          </div>
          <div class="sidebar-list" id="keyspaceList">
            <div class="list-empty">Not connected</div>
          </div>
        </div>

        <div class="sidebar-section">
          <div class="sidebar-header">
            Keys
            <button class="secondary small" id="refreshKeysBtn">Refresh</button>
          </div>
          <div style="padding: 0.5rem;">
            <input type="text" id="keyPrefix" placeholder="Filter by prefix..." style="width: 100%; font-size: 0.8rem; padding: 0.4rem;">
          </div>
          <div class="sidebar-list" id="keyList">
            <div class="list-empty">Select a keyspace</div>
          </div>
        </div>
      </div>

      <div class="main-content">
        <div class="tabs">
          <div class="tab active" data-tab="get">Get / Set Value</div>
          <div class="tab" data-tab="type">Register Type</div>
        </div>

        <div class="editor-area">
          <div id="tabContent">
            ${renderGetTab()}
          </div>

          <div class="binary-viewer" id="binaryViewer">
            <div class="binary-tabs">
              <div class="binary-tab active" data-tab="hexdump">Hexdump</div>
              <div class="binary-tab" data-tab="tree">Value Tree</div>
              <div class="binary-tab" data-tab="wave">WAVE</div>
              <div class="binary-tab" data-tab="json">JSON</div>
            </div>
            <div class="binary-content" id="binaryContent">
              <span style="color: var(--text-muted)">Select a key to view its value</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderGetTab(): string {
  return `
    <div class="editor-section">
      <h3>Value Editor</h3>
      <div class="form-row">
        <label>Keyspace:</label>
        <input type="text" id="valueKeyspace" value="${currentKeyspace || ''}" placeholder="keyspace">
      </div>
      <div class="form-row">
        <label>Key:</label>
        <input type="text" id="valueKey" value="${currentKey || ''}" placeholder="key">
      </div>
      <div class="form-row">
        <label></label>
        <div class="checkbox-row">
          <input type="checkbox" id="binaryFormat">
          <span>Binary format (shows hexdump, tree, and WAVE views)</span>
        </div>
      </div>
      <textarea id="valueContent" placeholder="WAVE format value, e.g.: {name: &quot;Alice&quot;, age: 30}"></textarea>
      <div class="form-row" style="margin-top: 0.75rem; gap: 0.5rem;">
        <button id="getBtn">Get</button>
        <button id="setBtn">Set</button>
        <button id="deleteBtn" class="secondary">Delete</button>
      </div>
      <div id="output" class="output" style="margin-top: 0.75rem; display: none;"></div>
    </div>
  `;
}

function renderTypeTab(): string {
  return `
    <div class="editor-section">
      <h3>Register Type</h3>
      <div class="form-row">
        <label>Keyspace:</label>
        <input type="text" id="typeKeyspace" value="${currentKeyspace || ''}" placeholder="keyspace name">
      </div>
      <div class="form-row">
        <label>Type Name:</label>
        <input type="text" id="typeName" placeholder="type name (optional, auto-detected)">
      </div>
      <textarea id="witDefinition" placeholder="package myapp:types@0.1.0;

interface types {
    record user {
        name: string,
        email: string,
        active: bool,
    }
}

world example {
    export types;
}"></textarea>
      <div class="form-row" style="margin-top: 0.75rem; gap: 0.5rem;">
        <button id="registerTypeBtn">Register Type</button>
        <button id="getTypeInfoBtn" class="secondary">Get Info</button>
        <button id="deleteTypeBtn" class="secondary">Delete Type</button>
      </div>
      <div id="typeOutput" class="output" style="margin-top: 0.75rem; display: none;"></div>
    </div>
  `;
}

function bindEvents() {
  // Database select
  document.getElementById('databaseSelect')?.addEventListener('change', (e) => {
    const select = e.target as HTMLSelectElement;
    currentDatabase = select.value;
    witKvService.setDatabase(currentDatabase);
    currentKeyspace = null;
    currentKey = null;
    refreshKeyspaces();
  });

  // Examples dropdown
  document.getElementById('examplesBtn')?.addEventListener('click', () => {
    const menu = document.getElementById('examplesMenu');
    menu?.classList.toggle('hidden');
  });

  // Close examples menu when clicking outside
  document.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    if (!target.closest('.examples-dropdown')) {
      document.getElementById('examplesMenu')?.classList.add('hidden');
    }
  });

  // Example items
  document.querySelectorAll('.example-item').forEach((item) => {
    item.addEventListener('click', () => {
      const keyspace = (item as HTMLElement).dataset.keyspace;
      if (keyspace) {
        loadExample(keyspace);
        document.getElementById('examplesMenu')?.classList.add('hidden');
      }
    });
  });

  // Refresh buttons
  document
    .getElementById('refreshKeyspacesBtn')
    ?.addEventListener('click', refreshKeyspaces);
  document
    .getElementById('refreshKeysBtn')
    ?.addEventListener('click', refreshKeys);

  // Key prefix filter
  document.getElementById('keyPrefix')?.addEventListener('input', () => {
    refreshKeys();
  });

  // Tabs
  document.querySelectorAll('.tabs .tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      const tabName = (tab as HTMLElement).dataset.tab as 'get' | 'type';
      switchTab(tabName);
    });
  });

  // Binary tabs
  document.querySelectorAll('.binary-tabs .binary-tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      const tabName = (tab as HTMLElement).dataset.tab as
        | 'hexdump'
        | 'tree'
        | 'wave'
        | 'json';
      switchBinaryTab(tabName);
    });
  });

  // Bind action buttons after tab switch
  bindTabEvents();
}

function bindTabEvents() {
  // Get/Set tab buttons
  document.getElementById('getBtn')?.addEventListener('click', getValue);
  document.getElementById('setBtn')?.addEventListener('click', setValue);
  document.getElementById('deleteBtn')?.addEventListener('click', deleteValue);

  // Type tab buttons
  document
    .getElementById('registerTypeBtn')
    ?.addEventListener('click', registerType);
  document
    .getElementById('getTypeInfoBtn')
    ?.addEventListener('click', getTypeInfo);
  document
    .getElementById('deleteTypeBtn')
    ?.addEventListener('click', deleteType);
}

function switchTab(tab: 'get' | 'type') {
  // Update tab UI
  document.querySelectorAll('.tabs .tab').forEach((t) => {
    t.classList.toggle('active', (t as HTMLElement).dataset.tab === tab);
  });

  // Render tab content
  const content = document.getElementById('tabContent');
  if (content) {
    content.innerHTML = tab === 'get' ? renderGetTab() : renderTypeTab();
    bindTabEvents();
  }
}

function switchBinaryTab(tab: 'hexdump' | 'tree' | 'wave' | 'json') {
  currentBinaryTab = tab;

  // Update tab UI
  document.querySelectorAll('.binary-tabs .binary-tab').forEach((t) => {
    t.classList.toggle('active', (t as HTMLElement).dataset.tab === tab);
  });

  // Update content
  updateBinaryView();
}

async function connect() {
  try {
    await witKvService.health();
    connected = true;
    updateConnectionStatus();

    // Load databases
    databases = await witKvService.listDatabases();
    updateDatabaseSelect();

    // Load keyspaces
    await refreshKeyspaces();
  } catch (err) {
    connected = false;
    updateConnectionStatus();
    throw err;
  }
}

function updateConnectionStatus() {
  const status = document.getElementById('connectionStatus');
  if (!status) return;

  if (connected) {
    status.className = 'status connected';
    status.innerHTML = '<span class="status-dot"></span>Connected';
  } else {
    status.className = 'status disconnected';
    status.innerHTML = '<span class="status-dot"></span>Disconnected';
  }
}

function updateDatabaseSelect() {
  const select = document.getElementById('databaseSelect') as HTMLSelectElement;
  if (!select) return;

  select.innerHTML = databases
    .map(
      (db) => `<option value="${db}" ${db === currentDatabase ? 'selected' : ''}>${db}</option>`
    )
    .join('');
}

async function refreshKeyspaces() {
  if (!connected) return;

  try {
    keyspaces = await witKvService.listKeyspaces();
    updateKeyspaceList();
  } catch (err) {
    console.error('Failed to load keyspaces:', err);
  }
}

function updateKeyspaceList() {
  const list = document.getElementById('keyspaceList');
  if (!list) return;

  if (keyspaces.length === 0) {
    list.innerHTML = '<div class="list-empty">No keyspaces</div>';
    return;
  }

  list.innerHTML = keyspaces
    .map(
      (ks) => `
    <div class="list-item ${ks.name === currentKeyspace ? 'selected' : ''}" data-keyspace="${ks.name}">
      <div class="name">${ks.name}</div>
      <div class="meta">${ks.typeName} v${ks.version}</div>
    </div>
  `
    )
    .join('');

  // Bind click events
  list.querySelectorAll('.list-item').forEach((item) => {
    item.addEventListener('click', () => {
      selectKeyspace((item as HTMLElement).dataset.keyspace || '');
    });
  });
}

function selectKeyspace(name: string) {
  currentKeyspace = name;
  currentKey = null;

  // Update UI
  updateKeyspaceList();

  // Update form
  const keyspaceInput = document.getElementById(
    'valueKeyspace'
  ) as HTMLInputElement;
  const typeKeyspaceInput = document.getElementById(
    'typeKeyspace'
  ) as HTMLInputElement;
  if (keyspaceInput) keyspaceInput.value = name;
  if (typeKeyspaceInput) typeKeyspaceInput.value = name;

  // Load keys
  refreshKeys();
}

async function refreshKeys() {
  if (!connected || !currentKeyspace) {
    const list = document.getElementById('keyList');
    if (list) {
      list.innerHTML = '<div class="list-empty">Select a keyspace</div>';
    }
    return;
  }

  try {
    const prefix =
      (document.getElementById('keyPrefix') as HTMLInputElement)?.value ||
      undefined;
    keys = await witKvService.listKeys(currentKeyspace, { prefix, limit: 100 });
    updateKeyList();
  } catch (err) {
    console.error('Failed to load keys:', err);
  }
}

function updateKeyList() {
  const list = document.getElementById('keyList');
  if (!list) return;

  if (keys.length === 0) {
    list.innerHTML = '<div class="list-empty">No keys</div>';
    return;
  }

  list.innerHTML = keys
    .map(
      (key) => `
    <div class="list-item ${key === currentKey ? 'selected' : ''}" data-key="${key}">
      <div class="name">${key}</div>
    </div>
  `
    )
    .join('');

  // Bind click events
  list.querySelectorAll('.list-item').forEach((item) => {
    item.addEventListener('click', () => {
      selectKey((item as HTMLElement).dataset.key || '');
    });
  });
}

function selectKey(key: string) {
  currentKey = key;

  // Update UI
  updateKeyList();

  // Update form
  const keyInput = document.getElementById('valueKey') as HTMLInputElement;
  if (keyInput) keyInput.value = key;

  // Automatically fetch the value
  getValue();
}

async function getValue() {
  const keyspace =
    (document.getElementById('valueKeyspace') as HTMLInputElement)?.value ||
    currentKeyspace;
  const key =
    (document.getElementById('valueKey') as HTMLInputElement)?.value ||
    currentKey;
  const binary = (document.getElementById('binaryFormat') as HTMLInputElement)
    ?.checked;

  if (!keyspace || !key) {
    showOutput('Please enter keyspace and key', 'error');
    return;
  }

  try {
    if (binary) {
      await getValueBinary(keyspace, key);
    } else {
      const value = await witKvService.getValue(keyspace, key);
      const textarea = document.getElementById(
        'valueContent'
      ) as HTMLTextAreaElement;
      if (textarea) textarea.value = value;
      showOutput('Value retrieved successfully', 'success');

      // Clear binary views
      currentBinaryData = null;
      currentValueTree = null;
      currentWaveText = value;
      updateBinaryView();
    }
  } catch (err) {
    showOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function getValueBinary(keyspace: string, key: string) {
  try {
    // Get binary data
    currentBinaryData = await witKvService.getValueBinary(keyspace, key);

    // Try to decode with wit-ast
    try {
      await witAstService.load();

      // Get type info
      const typeInfo = await witKvService.getTypeInfo(keyspace);

      // Parse WIT definition
      const ast = witAstService.parseWit(typeInfo.wit_definition);

      // Decode the binary-export envelope from the server response
      // The server sends a canonical ABI encoded binary-export record
      const binaryExport = decodeBinaryExportEnvelope(currentBinaryData);

      // Lift to value tree
      currentValueTree = witAstService.lift(
        ast,
        typeInfo.type_name,
        binaryExport
      );

      // Convert to WAVE
      currentWaveText = witAstService.valueTreeToWave(
        ast,
        typeInfo.type_name,
        currentValueTree
      );

      showOutput(
        `Binary value retrieved (${formatByteCount(currentBinaryData)})`,
        'success'
      );
    } catch (err) {
      console.error('Failed to decode binary:', err);
      currentValueTree = null;
      currentWaveText = null;
      // Extract error message from ComponentError payload if available
      const error = err as Error & { payload?: { message?: string; context?: string } };
      let errorMessage = error.message;
      if (error.payload?.message) {
        errorMessage = error.payload.message;
        if (error.payload.context) {
          errorMessage += ` (${error.payload.context})`;
        }
      }
      showOutput(
        `Binary retrieved but decoding failed: ${errorMessage}`,
        'error'
      );
    }

    updateBinaryView();
  } catch (err) {
    showOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

function updateBinaryView() {
  const content = document.getElementById('binaryContent');
  if (!content) return;

  switch (currentBinaryTab) {
    case 'hexdump':
      if (currentBinaryData) {
        content.innerHTML = `<pre>${formatHexdump(currentBinaryData)}</pre>`;
      } else {
        content.innerHTML =
          '<span style="color: var(--text-muted)">No binary data</span>';
      }
      break;

    case 'tree':
      if (currentValueTree) {
        content.innerHTML = `<pre>${renderValueTree(currentValueTree)}</pre>`;
      } else {
        content.innerHTML =
          '<span style="color: var(--text-muted)">No value tree (binary format required)</span>';
      }
      break;

    case 'wave':
      if (currentWaveText) {
        content.innerHTML = `<pre>${escapeHtml(currentWaveText)}</pre>`;
      } else {
        content.innerHTML =
          '<span style="color: var(--text-muted)">No WAVE text</span>';
      }
      break;

    case 'json':
      if (currentValueTree) {
        content.innerHTML = `<pre>${escapeHtml(valueTreeToJson(currentValueTree))}</pre>`;
      } else {
        content.innerHTML =
          '<span style="color: var(--text-muted)">No value tree (binary format required)</span>';
      }
      break;
  }
}

async function setValue() {
  const keyspace =
    (document.getElementById('valueKeyspace') as HTMLInputElement)?.value ||
    currentKeyspace;
  const key = (document.getElementById('valueKey') as HTMLInputElement)?.value;
  const value = (document.getElementById('valueContent') as HTMLTextAreaElement)
    ?.value;

  if (!keyspace || !key || !value) {
    showOutput('Please enter keyspace, key, and value', 'error');
    return;
  }

  try {
    await witKvService.setValue(keyspace, key, value);
    showOutput('Value set successfully', 'success');
    refreshKeys();
  } catch (err) {
    showOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function deleteValue() {
  const keyspace =
    (document.getElementById('valueKeyspace') as HTMLInputElement)?.value ||
    currentKeyspace;
  const key = (document.getElementById('valueKey') as HTMLInputElement)?.value;

  if (!keyspace || !key) {
    showOutput('Please enter keyspace and key', 'error');
    return;
  }

  try {
    await witKvService.deleteValue(keyspace, key);
    showOutput('Value deleted successfully', 'success');
    refreshKeys();
  } catch (err) {
    showOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function registerType() {
  const keyspace = (document.getElementById('typeKeyspace') as HTMLInputElement)
    ?.value;
  const typeName =
    (document.getElementById('typeName') as HTMLInputElement)?.value ||
    undefined;
  const witDef = (document.getElementById('witDefinition') as HTMLTextAreaElement)
    ?.value;

  if (!keyspace || !witDef) {
    showTypeOutput('Please enter keyspace and WIT definition', 'error');
    return;
  }

  try {
    const result = await witKvService.setType(keyspace, witDef, typeName);
    showTypeOutput(
      `Type registered:\n${JSON.stringify(result, null, 2)}`,
      'success'
    );
    refreshKeyspaces();
  } catch (err) {
    showTypeOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function getTypeInfo() {
  const keyspace = (document.getElementById('typeKeyspace') as HTMLInputElement)
    ?.value;

  if (!keyspace) {
    showTypeOutput('Please enter keyspace', 'error');
    return;
  }

  try {
    const info = await witKvService.getTypeInfo(keyspace);
    witKvService.clearTypeCache(); // Force fresh fetch next time
    showTypeOutput(JSON.stringify(info, null, 2), 'success');
  } catch (err) {
    showTypeOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function deleteType() {
  const keyspace = (document.getElementById('typeKeyspace') as HTMLInputElement)
    ?.value;

  if (!keyspace) {
    showTypeOutput('Please enter keyspace', 'error');
    return;
  }

  try {
    await witKvService.deleteType(keyspace, true);
    showTypeOutput('Type deleted successfully', 'success');
    refreshKeyspaces();
  } catch (err) {
    showTypeOutput(`Error: ${(err as Error).message}`, 'error');
  }
}

async function loadExample(keyspace: string) {
  const example = exampleCategories
    .flatMap((c) => c.examples)
    .find((e) => e.keyspace === keyspace);

  if (!example) return;

  try {
    // Ensure connected
    if (!connected) {
      await connect();
    }

    // Register the type
    await witKvService.setType(
      example.keyspace,
      example.witDefinition,
      example.typeName
    );

    // Set all example values
    for (const { key, value } of example.values) {
      await witKvService.setValue(example.keyspace, key, value);
    }

    // Refresh and select
    await refreshKeyspaces();
    selectKeyspace(example.keyspace);

    showOutput(
      `Loaded example: ${example.name} (${example.values.length} values)`,
      'success'
    );
  } catch (err) {
    showOutput(`Failed to load example: ${(err as Error).message}`, 'error');
  }
}

function showOutput(message: string, type: 'success' | 'error' = 'success') {
  const output = document.getElementById('output');
  if (output) {
    output.textContent = message;
    output.className = `output ${type}`;
    output.style.display = 'block';
  }
}

function showTypeOutput(
  message: string,
  type: 'success' | 'error' = 'success'
) {
  const output = document.getElementById('typeOutput');
  if (output) {
    output.textContent = message;
    output.className = `output ${type}`;
    output.style.display = 'block';
  }
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// Start the app
init();
