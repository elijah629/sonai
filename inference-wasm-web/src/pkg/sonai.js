import * as wasm from "./sonai_bg.wasm";
export * from "./sonai_bg.js";
import { __wbg_set_wasm } from "./sonai_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
