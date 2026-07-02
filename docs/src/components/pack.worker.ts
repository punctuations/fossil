import init, { Packer } from "../wasm/fossil_wasm.js";
import wasmUrl from "../wasm/fossil_wasm_bg.wasm?url";

const ready = init(wasmUrl);
const post = (m: unknown) => (self as unknown as Worker).postMessage(m);

self.onmessage = async (e: MessageEvent<Uint8Array>) => {
  await ready;

  const p = new Packer(e.data);
  const total = p.total();
  post({ type: "total", total });

  const batch = Math.max(1, Math.ceil(total / 60));
  let done = 0;
  while (done < total) {
    done = p.step(batch);
    post({ type: "progress", done, total });
  }

  const json = p.finish();
  p.free();
  post({ type: "done", json });
};
