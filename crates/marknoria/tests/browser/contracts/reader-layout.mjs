// Standalone entry; the same raw geometry checks are used by the public host.
export function layoutAssertions() {
  const evidence = []
  window.readerLayoutEvidence = evidence
  let assertions = 0
  const check = (ok, message) => { if (!ok) throw Error(message); assertions++ }
  const geometry = (samples, label) => {
    evidence.push({label,samples})
    for (const sample of samples) {
      check(sample.mounted <= 10, `${label}: mounted DOM budget grew`)
      for (const page of sample.pages.filter(page => page.ready))
        check(Math.abs(page.height-page.virtual) <= 1.1, `${label}: actual/virtual mismatch ${JSON.stringify(page)}`)
      for (let n=1;n<sample.pages.length;n++) {
        const a=sample.pages[n-1],b=sample.pages[n]
        if (a.ready && b.ready && b.index === a.index+1)
          check(Math.abs(b.top-a.bottom) <= 1.1, `${label}: overlap/gap ${b.top-a.bottom}: ${JSON.stringify([a,b])}`)
      }
    }
    return samples.at(-1)
  }
  return {check,geometry,report:()=>({assertions,evidence})}
}
export async function run() {
  const test = layoutAssertions(), probe = (action,params) => window.readerLayoutProbe(action,params)
  const first = test.geometry(await probe('sample'),'first-open')
  test.check(first.pages.some(page => page.table !== null),'Fixture did not mount its real table')
  const changed = test.geometry(await probe('font',{code:true,value:14}),'code-only-no-code')
  for (const page of first.pages) {
    const after = changed.pages.find(other => other.index === page.index)
    if (after) test.check(Math.abs(page.height-after.height)<1.1,'Code-only update changed prose/table size')
  }
  test.geometry(await probe('font',{code:true,value:14}),'same-code-size')
  test.geometry(await probe('theme',{theme:'dark'}),'theme-only')
  test.geometry(await probe('font',{value:19}),'body-size')
  test.geometry(await probe('width',{width:760}),'narrow-container')
  test.geometry(await probe('width',{width:1320}),'wide-container')
  test.geometry(await probe('scroll',{top:-1}),'scroll-out')
  test.geometry(await probe('scroll',{top:0}),'scroll-back')
  await probe('view',{view:'source'}); await probe('select')
  const selected = (await probe('geometry')).code
  const source = (await probe('font',{code:true,value:21})).at(-1)
  test.check(source.code.text === selected.text && source.code.from === selected.from && source.code.to === selected.to,'Source font update changed source/selection')
  await probe('view',{view:'edit'}); await probe('draft')
  const draft = (await probe('geometry')).code
  const after = (await probe('font',{code:true,value:18})).at(-1)
  test.check(after.dirty && after.code.text === draft.text && after.code.from === draft.from && after.code.to === draft.to,'Typography lost draft/caret')
  return {ok:true,mode:'standalone',...test.report()}
}
