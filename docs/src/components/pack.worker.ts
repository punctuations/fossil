import init, { pack_summary } from "../wasm/fossil_wasm.js";
import wasmUrl from "../wasm/fossil_wasm_bg.wasm?url";

const ready = init(wasmUrl);

self.onmessage = async (e: MessageEvent<Uint8Array>) => {
  await ready;
  const json = pack_summary(e.data);
  (self as unknown as Worker).postMessage(json);
};
