export async function fileUiContract(moduleUrl) {
  const { fileController } = await import(moduleUrl)
  const { createTranslator } = await import(new URL('./translation.mjs',new URL(moduleUrl,location.href)))
  const root=document.createElement('section')
  root.innerHTML='<button data-open-server>Open</button><button id="compare-files"></button><details class="reader-settings"></details><dialog id="file-browser"><h2></h2><div id="file-browser-list"></div><button id="file-browser-close"></button><button id="file-browser-cancel"></button><button id="file-browser-up"></button></dialog><div id="file-feedback"><span id="file-message"></span><button id="file-dismiss"></button></div><div id="welcome"><h2 id="welcome-title"></h2><p id="welcome-description"></p></div><div id="viewport"></div><span id="document-summary"></span><button id="contents-toggle"></button><button id="search-toggle"></button>'
  document.body.append(root)
  let opens=0,assertions=0
  const check=(value,message)=>{if(!value)throw Error(message);assertions++}
  const translate=createTranslator(), files=fileController('ui-contract',{translate,enabled:false,onOpen:()=>opens++})
  try {
    files.state('indexing')
    translate.update({locale:'fr',messages:{'Preparing your document…':'Préparation…','Could not open this document':'Ouverture impossible'}})
    check(root.querySelector('#welcome-title').textContent==='Préparation…','Preparing state did not update locale')
    files.state('error','Original backend error: 中文😀')
    translate.update({locale:'en',messages:{}})
    check(root.querySelector('#welcome-title').textContent==='Could not open this document','Error heading retained old locale')
    check(root.querySelector('#welcome-description').textContent==='Original backend error: 中文😀','Locale changed a literal backend error')
    document.dispatchEvent(new KeyboardEvent('keydown',{key:'o',ctrlKey:true,bubbles:true,cancelable:true}))
    check(opens===1,'Host Open shortcut did not execute exactly once')
    const handled=new KeyboardEvent('keydown',{key:'o',metaKey:true,bubbles:true,cancelable:true});handled.preventDefault();document.dispatchEvent(handled)
    check(opens===1,'Open shortcut ignored an already-handled event')
    root.querySelector('dialog').showModal()
    document.dispatchEvent(new KeyboardEvent('keydown',{key:'o',ctrlKey:true,bubbles:true,cancelable:true}))
    check(opens===1,'Open shortcut escaped the active modal')
    root.querySelector('dialog').close()
    files.dispose()
    translate.update({locale:'fr',messages:{'Could not open this document':'Wrong after disposal'}})
    check(root.querySelector('#welcome-title').textContent==='Could not open this document','Disposed file UI retained translation bindings')
  } finally {files.dispose();root.remove()}
  return {assertions}
}
