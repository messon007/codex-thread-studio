export async function tableControlsContract(moduleUrl) {
  const { tableControls } = await import(moduleUrl)
  const { createTranslator } = await import(
    new URL('./translation.mjs', new URL(moduleUrl, location.href))
  )
  const root = document.createElement('section')
  root.style.cssText = 'position:fixed;inset:12px;overflow:auto;background:white'
  document.body.append(root)
  const translate = createTranslator({
    locale: 'zh-CN',
    messages: {
      Table: '表格',
      Columns: '列',
      Copy: '复制',
      'Previous page': '上一页',
      'Next page': '下一页',
      'Visible columns': '可见列',
      'Find column…': '查找列…',
      'Find column': '查找列',
      'Select all': '全选',
      Clear: '清空',
      Apply: '应用',
      'Reset column widths': '恢复列宽',
      '{selected} of {total} selected': '已选择 {selected} / {total}',
      '{start}–{end} of {total} rows': '第 {start}–{end} 行，共 {total} 行'
    }
  })
  let applied,
    assertions = 0,
    source,
    generic
  const check = (value, message) => {
    if (!value) throw Error(message)
    assertions++
  }
  try {
    source = tableControls({
      names: ['Copy', 'Table', ''],
      selected: [0, 1, 2],
      label: 'Table',
      labelKey: null,
      translate,
      bounds: () => root,
      onApply: (value) => (applied = value)
    })
    generic = tableControls({
      names: ['Copy'],
      selected: [0],
      translate,
      bounds: () => root,
      onApply: () => {}
    })
    root.append(source.controls, generic.controls)
    source.counts(1, 50, 123456, 2469, [0, 1, 2])
    check(
      source.caption.querySelector('.table-label').textContent === 'Table' &&
        generic.caption.querySelector('.table-label').textContent === '表格',
      'A data caption was translated or a generic UI caption was missed'
    )
    check(
      [...source.controls.querySelectorAll('.column-name')]
        .map((node) => node.textContent)
        .join('|') === 'Copy|Table|(unnamed)',
      'Column data was translated'
    )
    check(
      source.previous.getAttribute('aria-label') === '上一页' &&
        source.picker.querySelector('summary').textContent.startsWith('列'),
      'Paging accessibility or menu label was not translated'
    )
    check(
      source.position.textContent === '第 1–50 行，共 123,456 行',
      'Row range or locale number formatting changed'
    )
    const checkboxes = source.picker.querySelectorAll('input[type="checkbox"]')
    checkboxes[1].checked = false
    checkboxes[1].dispatchEvent(new Event('change'))
    source.picker.querySelector('.column-panel-footer button').click()
    check(
      JSON.stringify(applied) === '[0,2]',
      'Translated Apply changed original column identities'
    )
    source.columns(applied)
    source.columns([0, 1, 2])
    check(checkboxes[1].checked, 'Navigation-revealed columns were not reflected in the picker')
    checkboxes[2].checked = false
    checkboxes[2].dispatchEvent(new Event('change'))
    source.counts(51, 100, 123456, 2469, [0, 1, 2])
    check(!checkboxes[2].checked, 'A row-only refresh reset an uncommitted column draft')
    translate.update({ locale: 'ar-EG', messages: { Columns: 'أعمدة' } })
    check(
      source.position.textContent.includes(new Intl.NumberFormat('ar-EG').format(123456)) &&
        source.pageCount.textContent.includes(new Intl.NumberFormat('ar-EG').format(2469)),
      'Existing row/page numbers retained another locale'
    )
    check(
      !checkboxes[2].checked &&
        source.caption.querySelector('.table-label').textContent === 'Table' &&
        source.controls.querySelector('.column-name').textContent === 'Copy',
      'Locale update changed column selection or literal names'
    )
    const before = source.position.textContent
    source.dispose()
    generic.dispose()
    translate.update({ locale: 'en' })
    check(
      translate.state().subscribers === 0 && source.position.textContent === before,
      'Disposed controls retained subscriptions or changed DOM'
    )
    return {
      assertions,
      translatedControls: true,
      literalNames: true,
      numericLocale: true,
      draftPreserved: true,
      restoredColumns: true,
      lifecycle: true
    }
  } finally {
    source?.dispose()
    generic?.dispose()
    root.remove()
  }
}
