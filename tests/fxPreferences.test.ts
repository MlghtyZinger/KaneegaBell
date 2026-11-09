import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import vm from "node:vm";

test("saved motion preferences load once even when both intros are disabled", async () => {
  let reads = 0;
  const context = vm.createContext({
    createRoot: (fn: () => unknown) => fn(),
    createSignal: (initial: unknown) => {
      let value = initial;
      return [() => value, (next: unknown) => { value = typeof next === "function" ? next(value) : next; }];
    },
    document: { documentElement: { dataset: {} } },
    localStorage: { getItem: () => "1", setItem: () => {} },
    api: {
      getSettingValues: async () => {
        reads++;
        return { introDesktop: "off", introTv: "off", animDesktop: "off", animTv: "reduced", tvMenu: "ps2" };
      },
    },
  });
  const source = stripTypeScriptTypes(readFileSync(new URL("../src/state/fx.ts", import.meta.url), "utf8")
    .replace(/^import .*;\r?\n/gm, "").replace(/\bexport /g, ""));
  vm.runInContext(source + "\nglobalThis.preferences = fx;", context);
  const fx = context.preferences;
  const first = fx.load();
  assert.equal(fx.load(), first);
  await first;
  assert.equal(reads, 1);
  assert.equal(fx.loaded(), true);
  assert.equal(fx.anim(), "off");
  fx.setTv(true);
  assert.equal(fx.anim(), "reduced");
  assert.equal(fx.get("tvMenu"), "ps2");
});
