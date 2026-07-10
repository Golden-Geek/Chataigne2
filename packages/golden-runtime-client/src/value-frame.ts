import { PROTOCOL_VERSION } from "./generated/protocol";

const HEADER_BYTES = 18;
const SAMPLE_BYTES = 12;

export interface DecodedValueFrame {
  sequence: number;
  generation: number;
  slots: Uint32Array;
  values: Float64Array;
}

export function decodeValueFrame(
  buffer: ArrayBuffer,
  maximumSamples = 1_000_000,
): DecodedValueFrame {
  if (buffer.byteLength < HEADER_BYTES) throw new Error("value frame is truncated");
  const bytes = new Uint8Array(buffer);
  if (bytes[0] !== 0x47 || bytes[1] !== 0x56 || bytes[2] !== 0x46 || bytes[3] !== 0x31) {
    throw new Error("value frame magic is invalid");
  }
  const view = new DataView(buffer);
  const version = view.getUint16(4, true);
  if (version !== PROTOCOL_VERSION) throw new Error(`unsupported value frame version ${version}`);
  const sequence = view.getUint32(6, true);
  const generation = view.getUint32(10, true);
  const count = view.getUint32(14, true);
  if (count > maximumSamples) throw new Error(`value frame sample limit exceeded: ${count}`);
  if (buffer.byteLength !== HEADER_BYTES + count * SAMPLE_BYTES) {
    throw new Error("value frame length does not match sample count");
  }
  const slots = new Uint32Array(count);
  const values = new Float64Array(count);
  for (let index = 0; index < count; index += 1) {
    const offset = HEADER_BYTES + index * SAMPLE_BYTES;
    slots[index] = view.getUint32(offset, true);
    const value = view.getFloat64(offset + 4, true);
    if (!Number.isFinite(value)) throw new Error("value frame contains a non-finite value");
    values[index] = value;
  }
  return { sequence, generation, slots, values };
}
