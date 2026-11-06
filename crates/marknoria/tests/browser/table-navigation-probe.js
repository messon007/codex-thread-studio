// Test-only child instrumentation, injected by the browser before reader startup.
// Navigation itself always uses the public protocol; no runtime hooks are added.
(() => {
  const { origins, parentOrigin, token } = __MARKNORIA_TABLE_TEST__
  if (window === parent || !origins.includes(location.origin)) return
  addEventListener('message', async event => {
    if (event.source !== parent || event.origin !== parentOrigin || event.data?.tableNavigationTest !== token) return
    const { id, action, row = 119, column = 1 } = event.data
    const geometry = () => {
      const viewport = document.getElementById('viewport'), article = document.querySelector('article[data-table="0"]')
      const box = viewport.getBoundingClientRect(), columns = JSON.parse(article?.dataset.selectedColumns || '[]')
      const cell = article?.querySelector('tbody')?.rows[row - Number(article.dataset.rowStart)]?.cells[columns.indexOf(column)]
      const area = article?.querySelector('.table-content'), bounds = area?.getBoundingClientRect(), cellBox = cell?.getBoundingClientRect()
      return {top:box.top+viewport.clientTop,left:box.left+viewport.clientLeft,width:viewport.clientWidth,height:viewport.clientHeight,
        scrollTop:viewport.scrollTop,scrollHeight:viewport.scrollHeight,ready:article?.dataset.ready,columns,
        cell:cellBox ? {x:cellBox.x,y:cellBox.y,width:cellBox.width,height:cellBox.height}:null,
        content:bounds?{left:bounds.left+area.clientLeft,width:area.clientWidth}:null,
        sourceReady:document.getElementById('source-view')?.dataset.ready,
        notes:document.querySelectorAll('.notes-open,.notes-toolbar').length}
    }
    try {
      const viewport = document.getElementById('viewport'), article = document.querySelector('article[data-table="0"]')
      if (action === 'hideColumn') {
        const picker = article.querySelector('.column-picker'); picker.open = true
        const choice = picker.querySelectorAll('input[type=checkbox]')[1]
        choice.checked = false; choice.dispatchEvent(new Event('change'))
        picker.querySelector('.column-panel-footer button').click()
      } else if (action === 'wheel') {
        viewport.dispatchEvent(new WheelEvent('wheel', {deltaY:-90,bubbles:true}))
        viewport.scrollTop = Math.max(0, viewport.scrollTop - 90)
      } else if (!['geometry','sample'].includes(action)) throw Error('Unknown test action')
      let result = geometry()
      if (action === 'sample') {
        // Consecutive child frames: protocol round trips cannot skip a bad paint.
        const samples = [{checkpoint:'return',geometry:result}]
        for (let frame=1;frame<=8;frame++) {await new Promise(requestAnimationFrame);samples.push({checkpoint:frame,geometry:geometry()})}
        await new Promise(resolve=>setTimeout(resolve,200));samples.push({checkpoint:'200ms',geometry:geometry()})
        result = {samples}
      }
      parent.postMessage({tableNavigationTest:token,id,result},event.origin)
    } catch (error) {parent.postMessage({tableNavigationTest:token,id,error:String(error)},event.origin)}
  })
})()
