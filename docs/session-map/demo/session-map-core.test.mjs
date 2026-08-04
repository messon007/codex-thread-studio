import test from "node:test";
import assert from "node:assert/strict";
import { applyDemoSync, currentPath, findItem, flattenItems, structures, validateCustomStructure } from "./session-map-core.mjs";

test("findItem and currentPath keep stable hierarchy identity", () => {
  const found = findItem(structures.hierarchy.items, "solver");
  assert.equal(found.item.title, "依赖求解器");
  assert.deepEqual(found.ancestors.map((item) => item.id), ["root", "packages", "apt"]);
  assert.deepEqual(currentPath(structures.hierarchy, "solver").map((item) => item.id), ["root", "packages", "apt", "solver"]);
});

test("flattenItems preserves display depth", () => {
  const flattened = flattenItems(structures.hierarchy.items);
  assert.equal(flattened.find((item) => item.id === "solver").depth, 3);
});

test("sync is immutable and idempotent in visible state", () => {
  const original = structuredClone(structures.hierarchy);
  const once = applyDemoSync(original);
  const twice = applyDemoSync(once);
  assert.equal(original.progress, 38);
  assert.equal(once.progress, 42);
  assert.equal(twice.progress, 42);
  assert.equal(flattenItems(twice.items).filter((item) => item.id === "solver").length, 1);
});

test("custom Structure validation requires a name, base, and view", () => {
  assert.deepEqual(validateCustomStructure({ name: "", base: "packages", views: [] }), ["请输入名称", "请选择有效基础结构", "至少选择一个视图"]);
  assert.deepEqual(validateCustomStructure({ name: "研究结构", base: "hierarchy", views: ["tree"] }), []);
});

