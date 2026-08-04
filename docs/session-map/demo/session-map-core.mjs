export const structureMeta = {
  hierarchy: { name: "层级", description: "章节、组件、主题、目录", icon: "hierarchy" },
  path: { name: "路径", description: "课程、研究、迁移、成长", icon: "path" },
  flow: { name: "流程", description: "开发、审核、内容、运营", icon: "flow" },
  blank: { name: "空白", description: "从最小结构开始", icon: "blank" }
};

const commonGoal = "了解系统升级项";

export const structures = {
  hierarchy: {
    id: "hierarchy",
    goal: commonGoal,
    progress: 38,
    currentItemId: "solver",
    views: [{ id: "tree", name: "树" }, { id: "outline", name: "纲" }],
    items: [
      { id: "root", title: "升级项", state: "active", meta: "3 / 8", children: [
        { id: "packages", title: "包管理", state: "active", meta: "", children: [
          { id: "dpkg", title: "dpkg", state: "visited", meta: "" },
          { id: "apt", title: "apt", state: "active", meta: "2 / 4", children: [
            { id: "solver", title: "依赖求解器", state: "active", meta: "" },
            { id: "sources", title: "软件源优先级", state: "notStarted", meta: "" }
          ]},
          { id: "unattended", title: "unattended-upgrades", state: "paused", meta: "" }
        ]},
        { id: "desktop", title: "桌面与图形", state: "notStarted", meta: "0 / 3" },
        { id: "development", title: "开发工具", state: "notStarted", meta: "0 / 2" }
      ]}
    ]
  },
  path: {
    id: "path",
    goal: commonGoal,
    progress: 40,
    currentItemId: "dependencies",
    views: [{ id: "path", name: "径" }, { id: "checklist", name: "单" }],
    items: [
      { id: "inventory", title: "整理升级清单", state: "done", meta: "" },
      { id: "roles", title: "理解核心职责", state: "visited", meta: "" },
      { id: "dependencies", title: "梳理依赖关系", state: "active", meta: "" },
      { id: "risks", title: "判断升级风险", state: "notStarted", meta: "" },
      { id: "decision", title: "确定升级顺序", state: "notStarted", meta: "" }
    ]
  },
  flow: {
    id: "flow",
    goal: commonGoal,
    progress: 36,
    currentItemId: "apt",
    views: [{ id: "board", name: "板" }, { id: "tree", name: "树" }],
    items: [
      { id: "dpkg", title: "dpkg", state: "done", meta: "" },
      { id: "apt", title: "apt", state: "active", meta: "" },
      { id: "unattended", title: "unattended-upgrades", state: "notStarted", meta: "" },
      { id: "mesa", title: "Mesa", state: "notStarted", meta: "" },
      { id: "gcc", title: "GCC", state: "paused", meta: "" }
    ]
  },
  blank: {
    id: "blank",
    goal: commonGoal,
    progress: 0,
    currentItemId: null,
    views: [{ id: "outline", name: "纲" }],
    items: []
  }
};

export function findItem(items, id, ancestors = []) {
  for (const item of items) {
    if (item.id === id) return { item, ancestors };
    const found = findItem(item.children || [], id, [...ancestors, item]);
    if (found) return found;
  }
  return null;
}

export function flattenItems(items, result = [], depth = 0) {
  for (const item of items) {
    result.push({ ...item, depth });
    flattenItems(item.children || [], result, depth + 1);
  }
  return result;
}

export function currentPath(structure, itemId) {
  if (!itemId) return [];
  const found = findItem(structure.items, itemId);
  return found ? [...found.ancestors, found.item].map((item) => ({ id: item.id, title: item.title })) : [];
}

export function applyDemoSync(structure) {
  const next = structuredClone(structure);
  if (next.id === "hierarchy") {
    const dpkg = findItem(next.items, "dpkg")?.item;
    if (dpkg) dpkg.state = "visited";
    const apt = findItem(next.items, "apt")?.item;
    if (apt && !findItem(next.items, "solver")) {
      apt.children ||= [];
      apt.children.push({ id: "solver", title: "依赖求解器", state: "active", meta: "当前" });
    }
    next.currentItemId = "solver";
    next.progress = 42;
  } else if (next.id === "path") {
    const roles = findItem(next.items, "roles")?.item;
    const dependencies = findItem(next.items, "dependencies")?.item;
    if (roles && roles.state !== "done") roles.state = "visited";
    if (dependencies) dependencies.state = "active";
    next.currentItemId = "dependencies";
    next.progress = 44;
  } else if (next.id === "flow") {
    const apt = findItem(next.items, "apt")?.item;
    if (apt) apt.state = "active";
    next.currentItemId = "apt";
    next.progress = 40;
  }
  return next;
}

export function validateCustomStructure(value) {
  const errors = [];
  if (!value || typeof value !== "object") return ["结构必须是对象"];
  if (!String(value.name || "").trim()) errors.push("请输入名称");
  if (!Object.hasOwn(structureMeta, value.base)) errors.push("请选择有效基础结构");
  if (!Array.isArray(value.views) || value.views.length === 0) errors.push("至少选择一个视图");
  return errors;
}
