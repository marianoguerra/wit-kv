/**
 * Format bytes as a hexdump with offset, hex bytes, and ASCII representation
 */
export function formatHexdump(buffer: ArrayBuffer, bytesPerLine = 16): string {
  const bytes = new Uint8Array(buffer);
  const lines: string[] = [];

  for (let offset = 0; offset < bytes.length; offset += bytesPerLine) {
    const chunk = bytes.slice(offset, offset + bytesPerLine);

    // Offset column
    const offsetStr = offset.toString(16).padStart(8, '0');

    // Hex bytes column
    const hexParts: string[] = [];
    for (let i = 0; i < bytesPerLine; i++) {
      if (i < chunk.length) {
        hexParts.push(chunk[i]!.toString(16).padStart(2, '0'));
      } else {
        hexParts.push('  ');
      }
      // Add extra space in the middle for readability
      if (i === 7) {
        hexParts.push('');
      }
    }
    const hexStr = hexParts.join(' ');

    // ASCII column
    const asciiParts: string[] = [];
    for (const byte of chunk) {
      if (byte >= 0x20 && byte < 0x7f) {
        asciiParts.push(String.fromCharCode(byte));
      } else {
        asciiParts.push('.');
      }
    }
    const asciiStr = asciiParts.join('');

    lines.push(`${offsetStr}  ${hexStr}  |${asciiStr}|`);
  }

  return lines.join('\n');
}

/**
 * Format bytes as a simple hex string
 */
export function formatHexString(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join(' ');
}

/**
 * Get byte count summary
 */
export function formatByteCount(buffer: ArrayBuffer): string {
  const bytes = buffer.byteLength;
  if (bytes < 1024) {
    return `${bytes} bytes`;
  } else if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  } else {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
}
