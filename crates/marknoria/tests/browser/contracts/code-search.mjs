export async function codeSearchContract(moduleUrl) {
  const { codeView } = await import(moduleUrl)
  const { createTranslator } = await import('/translation.mjs')
  const parent=document.createElement('div');parent.style.cssText='width:500px;height:350px';document.body.append(parent)
  const translate=createTranslator(), original='Unicode 中文😀\nneedle one\nneedle two'
  let assertions=0
  const check=(value,message)=>{if(!value)throw Error(message);assertions++}
  const editor=codeView(parent,{translate,maxBytes:512*1024})
  try {
    editor.load(original,10,'text'); editor.focus()
    const key=new KeyboardEvent('keydown',{key:'f',ctrlKey:true,bubbles:true,cancelable:true})
    editor.view.contentDOM.dispatchEvent(key)
    check(key.defaultPrevented && parent.querySelectorAll('.cm-search').length===1,'Editor did not own Ctrl+F')
    const query=parent.querySelector('input[name=search]');query.value='needle';query.dispatchEvent(new Event('change'))
    editor.view.dispatch({changes:{from:editor.text.length,insert:' draft'}})
    const before=editor.text, history=editor.canUndo
    translate.update({locale:'fr',messages:{Find:'Rechercher',Replace:'Remplacer',close:'Fermer'}})
    check(parent.querySelector('input[name=search]').getAttribute('aria-label')==='Rechercher','Open search UI did not change locale')
    check(parent.querySelector('input[name=search]').value==='needle','Locale update lost the active query')
    check(editor.text===before && editor.dirty && editor.canUndo===history,'Locale update changed the draft/history')
    check(editor.undo() && editor.text===original,'Search panel remount broke undo')
    parent.querySelector('input[name=search]').dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}))
    check(!parent.querySelector('.cm-search'),'Escape did not close the editor search panel')
    check(parent.contains(document.activeElement),'Search close lost editor focus')
  } finally {editor.dispose();parent.remove()}
  const readonly=document.createElement('div');document.body.append(readonly)
  const source=codeView(readonly,{readOnly:true,translate})
  try {
    source.load(original,1,'text');source.search()
    check(Boolean(readonly.querySelector('input[name=search]')),'Source has no search panel')
    check(!readonly.querySelector('input[name=replace]'),'Read-only Source exposed Replace')
    check(source.text===original && !source.dirty,'Source search changed original content')
  } finally {source.dispose();readonly.remove()}
  check(translate.state().subscribers===0,'Code search retained locale listeners after disposal')
  return {assertions}
}
