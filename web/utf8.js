// AudioWorkletGlobalScope does not expose these codecs in every browser.
if (!globalThis.TextDecoder) {
  globalThis.TextDecoder = class {
    decode(bytes = new Uint8Array()) {
      let out = '';
      for (let i = 0; i < bytes.length;) {
        const c = bytes[i++];
        if (c < 128) out += String.fromCharCode(c);
        else if (c < 224) out += String.fromCodePoint(((c & 31) << 6) | (bytes[i++] & 63));
        else if (c < 240) out += String.fromCodePoint(((c & 15) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63));
        else out += String.fromCodePoint(((c & 7) << 18) | ((bytes[i++] & 63) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63));
      }
      return out;
    }
  };
}
if (!globalThis.TextEncoder) {
  globalThis.TextEncoder = class {
    encode(text = '') {
      const out = [];
      for (const char of text) {
        const c = char.codePointAt(0);
        if (c < 128) out.push(c);
        else if (c < 2048) out.push(192 | (c >> 6), 128 | (c & 63));
        else if (c < 65536) out.push(224 | (c >> 12), 128 | ((c >> 6) & 63), 128 | (c & 63));
        else out.push(240 | (c >> 18), 128 | ((c >> 12) & 63), 128 | ((c >> 6) & 63), 128 | (c & 63));
      }
      return new Uint8Array(out);
    }
    encodeInto(text, buffer) {
      const bytes = this.encode(text);
      const written = Math.min(bytes.length, buffer.length);
      buffer.set(bytes.subarray(0, written));
      return { read: text.length, written };
    }
  };
}
