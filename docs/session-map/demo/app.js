import { applyDemoSync, currentPath, findItem, flattenItems, structureMeta, structures, validateCustomStructure } from "./session-map-core.mjs";

const $ = (selector) => document.querySelector(selector);
const icons = {
  hierarchy: '<svg viewBox="0 0 24 24"><path d="M12 4v5M6 20v-5h12v5M6 15v-4h12v4"/><circle cx="12" cy="4" r="2"/><circle cx="6" cy="20" r="2"/><circle cx="18" cy="20" r="2"/></svg>',
  path: '<svg viewBox="0 0 24 24"><circle cx="6" cy="18" r="2"/><circle cx="18" cy="6" r="2"/><path d="M7.5 16.5 16.5 7.5"/><path d="M8 6h4M10 4v4"/></svg>',
  flow: '<svg viewBox="0 0 24 24"><rect x="3" y="5" width="5" height="14" rx="1"/><rect x="10" y="5" width="5" height="9" rx="1"/><rect x="17" y="5" width="4" height="6" rx="1"/></svg>',
  blank: '<svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M12 8v8M8 12h8"/></svg>',
  tree: '<svg viewBox="0 0 24 24"><path d="M8 5h11M8 12h11M8 19h11"/><circle cx="4" cy="5" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="19" r="1"/></svg>',
  outline: '<svg viewBox="0 0 24 24"><path d="M9 6h10M9 12h10M9 18h10M4 6h1M4 12h1M4 18h1"/></svg>',
  board: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="5" height="16" rx="1"/><rect x="10" y="4" width="5" height="10" rx="1"/><rect x="17" y="4" width="4" height="13" rx="1"/></svg>',
  checklist: '<svg viewBox="0 0 24 24"><path d="m4 6 2 2 3-4M11 6h9M4 13l2 2 3-4M11 13h9M4 20l2 2 3-4M11 20h9"/></svg>',
  pathView: '<svg viewBox="0 0 24 24"><circle cx="6" cy="18" r="2"/><circle cx="18" cy="6" r="2"/><path d="M7.5 16.5 16.5 7.5"/></svg>',
  continue: '<svg viewBox="0 0 24 24"><path d="M5 12h14M14 7l5 5-5 5"/></svg>',
  back: '<svg viewBox="0 0 24 24"><path d="M19 12H5M10 7l-5 5 5 5"/></svg>',
  rename: '<svg viewBox="0 0 24 24"><path d="m4 20 4.5-1 10-10a2.1 2.1 0 0 0-3-3l-10 10L4 20Z"/><path d="m14 7 3 3"/></svg>',
  move: '<svg viewBox="0 0 24 24"><path d="M12 3v18M3 12h18M8 7l4-4 4 4M8 17l4 4 4-4M7 8l-4 4 4 4M17 8l4 4-4 4"/></svg>',
  state: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16Z"/></svg>',
  remove: '<svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5"/></svg>'
};

const state = {
  structureId: "hierarchy",
  data: structuredClone(structures.hierarchy),
  original: structuredClone(structures.hierarchy),
  viewId: "tree",
  selectedId: "solver",
  structureChoice: "hierarchy",
  syncApplied: false,
  collapsed: new Set(),
  openItemMenuId: null
};

const mapContent = $("#map-content");
const viewMenu = $("#view-menu");
const moreMenu = $("#more-menu");
let toastTimer;

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>'"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[character]);
}

function showToast(message) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.add("visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove("visible"), 2400);
}

function closePopovers(except) {
  for (const menu of [viewMenu, moreMenu]) if (menu !== except) menu.classList.add("hidden");
  $("#view-button").setAttribute("aria-expanded", String(except === viewMenu && !viewMenu.classList.contains("hidden")));
  $("#more-button").setAttribute("aria-expanded", String(except === moreMenu && !moreMenu.classList.contains("hidden")));
}

function togglePopover(menu, button) {
  const opening = menu.classList.contains("hidden");
  closePopovers();
  menu.classList.toggle("hidden", !opening);
  button.setAttribute("aria-expanded", String(opening));
}

function stateSymbol(value) {
  if (value === "done" || value === "visited") return "✓";
  if (value === "paused") return "–";
  return "";
}

function stateLabel(value) {
  return ({ notStarted: "未开始", active: "当前", visited: "已浏览", done: "完成", paused: "暂停" })[value] || "未知状态";
}

function stateMark(item) {
  const label = stateLabel(item.state);
  return `<span class="state-mark ${item.state}" title="${label}" aria-label="${label}">${stateSymbol(item.state)}</span>`;
}

function rowTail(item) {
  const count = item.children?.length || 0;
  const expanded = state.openItemMenuId === item.id;
  return `<span class="row-tail">${count ? `<span class="item-count">${count}</span>` : ""}<button class="row-more" type="button" data-item-menu-id="${item.id}" aria-label="${escapeHtml(item.title)} 操作" aria-expanded="${expanded}">•••</button></span>`;
}

function itemActionMenu(itemId) {
  if (state.openItemMenuId !== itemId) return "";
  return `<div class="item-action-menu" role="menu" aria-label="条目操作">
    <button type="button" role="menuitem" data-action="continue" data-item-id="${itemId}">${icons.continue}<span>继续</span></button>
    <button type="button" role="menuitem" data-action="back" data-item-id="${itemId}">${icons.back}<span>返回</span></button>
    <span class="menu-separator"></span>
    <button type="button" role="menuitem" data-action="rename" data-item-id="${itemId}">${icons.rename}<span>重命名</span></button>
    <button type="button" role="menuitem" data-action="move" data-item-id="${itemId}">${icons.move}<span>移动</span></button>
    <button type="button" role="menuitem" data-action="state" data-item-id="${itemId}">${icons.state}<span>状态</span></button>
    <span class="menu-separator"></span>
    <button class="danger" type="button" role="menuitem" data-action="remove" data-item-id="${itemId}">${icons.remove}<span>删除</span></button>
  </div>`;
}

function treeMarkup(items, level = 1) {
  return `<ul class="item-list">${items.map((item) => {
    const children = item.children || [];
    const selected = item.id === state.selectedId;
    const collapsed = state.collapsed.has(item.id);
    return `<li class="item-node">
      <div class="item-row ${selected ? "selected" : ""} ${collapsed ? "collapsed" : ""}" data-item-id="${item.id}" role="treeitem" aria-level="${level}" aria-selected="${selected}">
        <button class="disclosure ${children.length ? "" : "placeholder"}" type="button" data-collapse-id="${item.id}" aria-label="${children.length ? "展开或折叠" : "无子项"}">⌄</button>
        ${stateMark(item)}
        <span class="item-copy"><strong>${escapeHtml(item.title)}</strong>${item.meta ? `<small>${escapeHtml(item.meta)}</small>` : ""}</span>
        ${rowTail(item)}
      </div>
      ${itemActionMenu(item.id)}
      ${children.length && !collapsed ? treeMarkup(children, level + 1) : ""}
    </li>`;
  }).join("")}</ul>`;
}

function outlineMarkup() {
  return `<ol class="outline-list">${flattenItems(state.data.items).map((item, index) => `<li class="item-node"><div class="outline-row ${item.id === state.selectedId ? "selected" : ""}" data-item-id="${item.id}" style="padding-left:${8 + item.depth * 13}px"><span class="outline-index">${String(index + 1).padStart(2, "0")}</span><span><strong>${escapeHtml(item.title)}</strong>${item.meta ? `<small> · ${escapeHtml(item.meta)}</small>` : ""}</span>${stateMark(item)}${rowTail(item)}</div>${itemActionMenu(item.id)}</li>`).join("")}</ol>`;
}

function pathMarkup() {
  return `<div class="path-view">${state.data.items.map((item, index) => `<div class="item-node"><div class="path-step ${item.state} ${item.id === state.selectedId ? "selected" : ""}" data-item-id="${item.id}"><span class="path-number">${item.state === "done" ? "✓" : index + 1}</span><div><strong>${escapeHtml(item.title)}</strong><small>${escapeHtml(item.meta || "")}</small></div>${rowTail(item)}</div>${itemActionMenu(item.id)}</div>`).join("")}</div>`;
}

function checklistMarkup() {
  return `<div class="checklist-view">${state.data.items.map((item) => `<div class="item-node"><div class="check-row ${item.id === state.selectedId ? "selected" : ""}" data-item-id="${item.id}">${stateMark(item)}<strong>${escapeHtml(item.title)}</strong>${rowTail(item)}</div>${itemActionMenu(item.id)}</div>`).join("")}</div>`;
}

function boardMarkup() {
  const flat = flattenItems(state.data.items);
  const columns = [
    { title: "稍后", states: ["notStarted", "paused"] },
    { title: "进行中", states: ["active"] },
    { title: "完成", states: ["visited", "done"] }
  ];
  return `<div class="board-view">${columns.map((column) => {
    const items = flat.filter((item) => column.states.includes(item.state));
    return `<section class="board-column"><h3>${column.title}<span>${items.length}</span></h3>${items.map((item) => `<div class="board-card item-node ${item.id === state.selectedId ? "selected" : ""}" data-item-id="${item.id}"><strong>${escapeHtml(item.title)}</strong><small>${escapeHtml(item.meta || "")}</small><button class="row-more" type="button" data-item-menu-id="${item.id}" aria-label="${escapeHtml(item.title)} 操作" aria-expanded="${state.openItemMenuId === item.id}">•••</button>${itemActionMenu(item.id)}</div>`).join("")}</section>`;
  }).join("")}</div>`;
}

function blankMarkup() {
  return `<div class="blank-state"><span>＋</span><strong>空白 Map</strong><p>添加第一项，或从其他结构重新开始。</p><button id="blank-add" class="primary-button" type="button">添加</button></div>`;
}

function renderViewMenu() {
  const iconFor = (id) => id === "path" ? icons.pathView : icons[id] || icons.outline;
  viewMenu.innerHTML = state.data.views.map((view) => `<button type="button" role="menuitem" data-view-id="${view.id}" class="${view.id === state.viewId ? "active" : ""}">${iconFor(view.id)}<span>${view.name}</span></button>`).join("");
}

function renderBreadcrumb() {
  const path = currentPath(state.data, state.selectedId);
  $("#breadcrumb").innerHTML = path.length
    ? path.map((item, index) => `${index ? "<i>/</i>" : ""}<button type="button" data-breadcrumb-id="${item.id}">${escapeHtml(item.title)}</button>`).join("")
    : "未选择";
}

function render() {
  $("#map-goal").textContent = state.data.goal;
  $("#map-percent").textContent = `${state.data.progress}%`;
  $("#map-progress-bar").style.width = `${state.data.progress}%`;
  renderBreadcrumb();
  renderViewMenu();
  if (!state.data.items.length) mapContent.innerHTML = blankMarkup();
  else if (state.viewId === "outline") mapContent.innerHTML = outlineMarkup();
  else if (state.viewId === "path") mapContent.innerHTML = pathMarkup();
  else if (state.viewId === "checklist") mapContent.innerHTML = checklistMarkup();
  else if (state.viewId === "board") mapContent.innerHTML = boardMarkup();
  else mapContent.innerHTML = treeMarkup(state.data.items);
  const sync = $("#sync-status");
  sync.classList.toggle("updated", state.syncApplied);
  sync.innerHTML = state.syncApplied ? '<span class="sync-mark">✓</span><span>已同步 · 撤销</span>' : '<span class="sync-mark">✦</span><span>演示同步</span>';
}

function selectItem(itemId) {
  if (!findItem(state.data.items, itemId)) return;
  state.selectedId = itemId;
  state.data.currentItemId = itemId;
  state.openItemMenuId = null;
  render();
}

function prepareDraft(action, itemId) {
  const found = findItem(state.data.items, itemId);
  if (!found) return;
  const path = [...found.ancestors, found.item].map((item) => item.title).join(" / ");
  const parent = found.ancestors.at(-1)?.title || "总体目标";
  $("#composer-input").value = action === "back"
    ? `请归纳“${found.item.title}”的结论和遗留问题，然后回到“${parent}”。`
    : `继续“${path}”，先处理当前未完成的内容。`;
  $("#composer-input").focus();
  showToast("草稿已准备，确认后发送");
}

function performSync() {
  state.openItemMenuId = null;
  if (!state.syncApplied) {
    state.original = structuredClone(state.data);
    state.data = applyDemoSync(state.data);
    state.selectedId = state.data.currentItemId;
    state.syncApplied = true;
    render();
    showToast("Map 已更新");
  } else {
    state.data = structuredClone(state.original);
    state.selectedId = state.data.currentItemId;
    state.syncApplied = false;
    render();
    showToast("已撤销");
  }
}

function structureCard(id) {
  const meta = structureMeta[id];
  return `<button class="structure-card ${id === state.structureChoice ? "selected" : ""}" type="button" data-structure-id="${id}"><span class="structure-icon">${icons[meta.icon]}</span><strong>${meta.name}</strong><small>${meta.description}</small></button>`;
}

function openStructureDialog() {
  state.structureChoice = state.structureId;
  $("#structure-grid").innerHTML = Object.keys(structureMeta).map(structureCard).join("");
  $("#structure-dialog").showModal();
  closePopovers();
}

mapContent.addEventListener("click", (event) => {
  const menuButton = event.target.closest("[data-item-menu-id]");
  if (menuButton) {
    event.stopPropagation();
    const id = menuButton.dataset.itemMenuId;
    state.selectedId = id;
    state.data.currentItemId = id;
    state.openItemMenuId = state.openItemMenuId === id ? null : id;
    render();
    return;
  }
  const collapse = event.target.closest("[data-collapse-id]");
  if (collapse) {
    event.stopPropagation();
    const id = collapse.dataset.collapseId;
    if (!findItem(state.data.items, id)?.item.children?.length) return;
    state.collapsed.has(id) ? state.collapsed.delete(id) : state.collapsed.add(id);
    render();
    return;
  }
  const action = event.target.closest("[data-action]");
  if (action) {
    event.stopPropagation();
    const actionName = action.dataset.action;
    state.openItemMenuId = null;
    if (actionName === "continue" || actionName === "back") prepareDraft(actionName, action.dataset.itemId);
    else showToast({ rename: "重命名", move: "移动", state: "更改状态", remove: "删除需要确认" }[actionName] || "条目操作");
    render();
    return;
  }
  const item = event.target.closest("[data-item-id]");
  if (item) selectItem(item.dataset.itemId);
  if (event.target.id === "blank-add") showToast("真实产品会在此创建第一项");
});

$("#breadcrumb").addEventListener("click", (event) => {
  const item = event.target.closest("[data-breadcrumb-id]");
  if (item) selectItem(item.dataset.breadcrumbId);
});
$("#view-button").addEventListener("click", () => togglePopover(viewMenu, $("#view-button")));
$("#more-button").addEventListener("click", () => togglePopover(moreMenu, $("#more-button")));
viewMenu.addEventListener("click", (event) => {
  const button = event.target.closest("[data-view-id]");
  if (!button) return;
  state.viewId = button.dataset.viewId;
  state.openItemMenuId = null;
  closePopovers();
  render();
});
$("#choose-structure").addEventListener("click", openStructureDialog);
$("#run-sync").addEventListener("click", () => { closePopovers(); performSync(); });
$("#sync-status").addEventListener("click", performSync);
$("#show-history").addEventListener("click", () => { closePopovers(); showToast(state.syncApplied ? "最近：Map 已同步" : "尚无更新"); });
$("#add-item").addEventListener("click", () => showToast("真实产品会在当前层级添加条目"));

const structureDialog = $("#structure-dialog");
$("#structure-grid").addEventListener("click", (event) => {
  const card = event.target.closest("[data-structure-id]");
  if (!card) return;
  state.structureChoice = card.dataset.structureId;
  $("#structure-grid").innerHTML = Object.keys(structureMeta).map(structureCard).join("");
});
$("#close-structure").addEventListener("click", () => structureDialog.close());
$("#structure-form").addEventListener("submit", (event) => {
  event.preventDefault();
  state.structureId = state.structureChoice;
  state.data = structuredClone(structures[state.structureId]);
  state.original = structuredClone(state.data);
  state.viewId = state.data.views[0].id;
  state.selectedId = state.data.currentItemId;
  state.syncApplied = false;
  state.collapsed.clear();
  state.openItemMenuId = null;
  structureDialog.close();
  render();
});

const customDialog = $("#custom-dialog");
$("#custom-structure").addEventListener("click", () => { structureDialog.close(); customDialog.showModal(); });
$("#close-custom").addEventListener("click", () => customDialog.close());
$("#custom-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const candidate = {
    name: $("#custom-name").value,
    base: $("#custom-base").value,
    views: [...event.currentTarget.querySelectorAll("fieldset input:checked")].map((input) => input.value),
    sync: $("#custom-sync").checked
  };
  const errors = validateCustomStructure(candidate);
  if (errors.length) { $("#custom-validation").textContent = errors[0]; return; }
  localStorage.setItem("session-map-demo-structure", JSON.stringify(candidate));
  customDialog.close();
  showToast("结构已保存到 Demo");
});

$("#composer").addEventListener("submit", (event) => { event.preventDefault(); showToast("Demo 不连接 Codex"); });
document.addEventListener("click", (event) => {
  if (!event.target.closest(".menu-anchor")) closePopovers();
  if (state.openItemMenuId && !event.target.closest(".item-action-menu") && !event.target.closest(".row-more")) {
    state.openItemMenuId = null;
    render();
  }
});

render();
