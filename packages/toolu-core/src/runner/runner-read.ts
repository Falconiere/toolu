/** Read subprocess streams with a byte ceiling. */

async function readChunk(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  chunks: Uint8Array[],
  state: { total: number; truncated: boolean },
  maxBytes: number,
): Promise<void> {
  const { done, value } = await reader.read();
  if (done) {
    return;
  }
  if (value.byteLength > 0) {
    if (state.total >= maxBytes) {
      state.truncated = true;
    } else {
      const room = maxBytes - state.total;
      if (value.byteLength <= room) {
        chunks.push(value);
        state.total += value.byteLength;
      } else {
        chunks.push(value.subarray(0, room));
        state.total = maxBytes;
        state.truncated = true;
      }
    }
  }
  await readChunk(reader, chunks, state, maxBytes);
}

export async function readLimitedStream(
  stream: ReadableStream<Uint8Array>,
  maxBytes: number,
): Promise<{ text: string; truncated: boolean }> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  const state = { total: 0, truncated: false };
  await readChunk(reader, chunks, state, maxBytes);
  const merged = new Uint8Array(state.total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return { text: new TextDecoder().decode(merged), truncated: state.truncated };
}
