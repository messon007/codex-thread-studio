// Temporary Studio compatibility layer for MarkNoria #38; remove on fixed SDK adoption.
function remeasureMountedPages() {
  if (!virtualizer) return
  virtualizer.measure()
  virtualizer.getVirtualItems()
  for (const [index, node] of nodes) {
    if (node.isConnected && node.dataset.ready === 'true' && !node.classList.contains('placeholder')) {
      virtualizer.resizeItem(index, virtualizer.options.measureElement(node, undefined, virtualizer))
    }
  }
  commitVirtualGeometry()
}

