import type { ValueTree, WitValueNode, PrimitiveValue } from '../services/wit-ast-service';

/**
 * Render a ValueTree as an interactive HTML tree
 */
export function renderValueTree(tree: ValueTree): string {
  if (tree.nodes.length === 0) {
    return '<span class="tree-value">(empty)</span>';
  }

  const expandedNodes = new Set<number>([0]); // Root expanded by default
  return renderNode(tree.nodes, 0, 0, expandedNodes);
}

function renderNode(
  nodes: WitValueNode[],
  index: number,
  depth: number,
  expanded: Set<number>
): string {
  const node = nodes[index];
  if (!node) {
    return '<span class="tree-value error">(invalid node)</span>';
  }

  const indent = '  '.repeat(depth);

  switch (node.tag) {
    case 'primitive':
      return renderPrimitive(node.val);

    case 'record-val': {
      if (node.val.length === 0) {
        return '<span class="tree-type">record</span> {}';
      }
      const fields = node.val
        .map((field) => {
          const value = renderNode(nodes, field.valueIdx, depth + 1, expanded);
          return `${indent}  <span class="tree-key">${escapeHtml(field.name)}</span>: ${value}`;
        })
        .join(',\n');
      return `<span class="tree-type">record</span> {\n${fields}\n${indent}}`;
    }

    case 'tuple-val': {
      if (node.val.length === 0) {
        return '<span class="tree-type">tuple</span> ()';
      }
      const items = Array.from(node.val)
        .map((idx) => `${indent}  ${renderNode(nodes, idx, depth + 1, expanded)}`)
        .join(',\n');
      return `<span class="tree-type">tuple</span> (\n${items}\n${indent})`;
    }

    case 'list-val': {
      if (node.val.length === 0) {
        return '<span class="tree-type">list</span> []';
      }
      const items = Array.from(node.val)
        .map((idx) => `${indent}  ${renderNode(nodes, idx, depth + 1, expanded)}`)
        .join(',\n');
      return `<span class="tree-type">list</span> [\n${items}\n${indent}]`;
    }

    case 'enum-val':
      return `<span class="tree-type">enum</span> <span class="tree-value">${escapeHtml(node.val)}</span>`;

    case 'variant-val': {
      if (node.val.payloadIdx !== undefined) {
        const payload = renderNode(nodes, node.val.payloadIdx, depth + 1, expanded);
        return `<span class="tree-type">variant</span> <span class="tree-key">${escapeHtml(node.val.name)}</span>(${payload})`;
      }
      return `<span class="tree-type">variant</span> <span class="tree-key">${escapeHtml(node.val.name)}</span>`;
    }

    case 'option-val':
      if (node.val === undefined) {
        return '<span class="tree-type">option</span> <span class="tree-value">none</span>';
      }
      return `<span class="tree-type">option</span> some(${renderNode(nodes, node.val, depth + 1, expanded)})`;

    case 'result-val':
      if (node.val.tag === 'ok') {
        if (node.val.val !== undefined) {
          return `<span class="tree-type">result</span> <span class="tree-value">ok</span>(${renderNode(nodes, node.val.val, depth + 1, expanded)})`;
        }
        return '<span class="tree-type">result</span> <span class="tree-value">ok</span>';
      } else {
        if (node.val.val !== undefined) {
          return `<span class="tree-type">result</span> <span class="tree-value error">err</span>(${renderNode(nodes, node.val.val, depth + 1, expanded)})`;
        }
        return '<span class="tree-type">result</span> <span class="tree-value error">err</span>';
      }

    case 'flags-val':
      if (node.val.length === 0) {
        return '<span class="tree-type">flags</span> {}';
      }
      return `<span class="tree-type">flags</span> { ${node.val.map((f) => `<span class="tree-value">${escapeHtml(f)}</span>`).join(', ')} }`;

    default:
      return `<span class="tree-value error">(unknown node type)</span>`;
  }
}

function renderPrimitive(val: PrimitiveValue): string {
  switch (val.tag) {
    case 'bool-val':
      return `<span class="tree-value">${val.val}</span>`;
    case 'u8-val':
    case 'u16-val':
    case 'u32-val':
    case 's8-val':
    case 's16-val':
    case 's32-val':
    case 'f32-val':
    case 'f64-val':
      return `<span class="tree-value number">${val.val}</span>`;
    case 'u64-val':
    case 's64-val':
      return `<span class="tree-value number">${val.val.toString()}</span>`;
    case 'char-val':
      return `<span class="tree-value string">'${escapeHtml(val.val)}'</span>`;
    case 'string-val':
      return `<span class="tree-value string">"${escapeHtml(val.val)}"</span>`;
    default:
      return '<span class="tree-value">(unknown)</span>';
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

/**
 * Render ValueTree as JSON (for debugging)
 */
export function valueTreeToJson(tree: ValueTree): string {
  return JSON.stringify(
    tree,
    (_key, value) => {
      if (typeof value === 'bigint') {
        return value.toString() + 'n';
      }
      if (value instanceof Uint32Array) {
        return Array.from(value);
      }
      return value;
    },
    2
  );
}
