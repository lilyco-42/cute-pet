'use strict';

(function exposeZipStore(root) {
  const MAX_ENTRIES = 65535;
  const MAX_ARCHIVE_BYTES = 32 * 1024 * 1024;
  const encoder = new TextEncoder();

  function crc32(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) {
      crc ^= byte;
      for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
    return (crc ^ 0xffffffff) >>> 0;
  }

  function zipHeader(size, write) {
    const bytes = new Uint8Array(size);
    write(new DataView(bytes.buffer));
    return bytes;
  }

  function validateEntryName(name) {
    if (typeof name !== 'string' || !name || name.startsWith('/') || name.includes('\\') || name.includes(':') ||
      name.split('/').some(part => !part || part === '.' || part === '..')) {
      throw new Error('ZIP 文件路径无效');
    }
    const bytes = encoder.encode(name);
    if (bytes.length > 65535) throw new Error('ZIP 文件名过长');
    return bytes;
  }

  async function createStoredZip(entries) {
    if (!Array.isArray(entries) || entries.length < 1 || entries.length > MAX_ENTRIES) {
      throw new Error('ZIP 文件数量无效');
    }
    const seen = new Set();
    let totalBytes = 0;
    const prepared = [];
    for (const entry of entries) {
      if (!entry || !(entry.blob instanceof Blob)) throw new Error('ZIP 文件内容必须是 Blob');
      const nameBytes = validateEntryName(entry.name);
      if (seen.has(entry.name)) throw new Error('ZIP 包含重复路径');
      seen.add(entry.name);
      totalBytes += entry.blob.size;
      if (totalBytes > MAX_ARCHIVE_BYTES) throw new Error('ZIP 文件总大小超过 32 MB');
      prepared.push({ name: nameBytes, blob: entry.blob });
    }

    const localParts = [];
    const centralParts = [];
    let offset = 0;
    for (const entry of prepared) {
      const data = new Uint8Array(await entry.blob.arrayBuffer());
      const checksum = crc32(data);
      const local = zipHeader(30, view => {
        view.setUint32(0, 0x04034b50, true); view.setUint16(4, 20, true); view.setUint16(6, 0x0800, true);
        view.setUint16(8, 0, true); view.setUint16(10, 0, true); view.setUint16(12, 0x21, true);
        view.setUint32(14, checksum, true); view.setUint32(18, data.length, true); view.setUint32(22, data.length, true);
        view.setUint16(26, entry.name.length, true); view.setUint16(28, 0, true);
      });
      localParts.push(local, entry.name, data);
      const central = zipHeader(46, view => {
        view.setUint32(0, 0x02014b50, true); view.setUint16(4, 20, true); view.setUint16(6, 20, true);
        view.setUint16(8, 0x0800, true); view.setUint16(10, 0, true); view.setUint16(12, 0, true); view.setUint16(14, 0x21, true);
        view.setUint32(16, checksum, true); view.setUint32(20, data.length, true); view.setUint32(24, data.length, true);
        view.setUint16(28, entry.name.length, true); view.setUint16(30, 0, true); view.setUint16(32, 0, true);
        view.setUint16(34, 0, true); view.setUint16(36, 0, true); view.setUint32(38, 0, true); view.setUint32(42, offset, true);
      });
      centralParts.push(central, entry.name);
      offset += local.length + entry.name.length + data.length;
      if (offset > 0xffffffff) throw new Error('ZIP 文件总大小超出格式限制');
    }
    const centralSize = centralParts.reduce((sum, part) => sum + part.length, 0);
    const end = zipHeader(22, view => {
      view.setUint32(0, 0x06054b50, true); view.setUint16(4, 0, true); view.setUint16(6, 0, true);
      view.setUint16(8, prepared.length, true); view.setUint16(10, prepared.length, true);
      view.setUint32(12, centralSize, true); view.setUint32(16, offset, true); view.setUint16(20, 0, true);
    });
    return new Blob([...localParts, ...centralParts, end], { type: 'application/zip' });
  }

  root.Lain42Zip = Object.freeze({ createStoredZip });
})(globalThis);
