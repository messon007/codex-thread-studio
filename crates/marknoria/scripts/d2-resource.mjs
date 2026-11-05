// Reviewed adapter for @d2lang/d2 0.1.34 (MPL-2.0).
// Keep this exact-match patch beside the resource builder: upgrades must review it.
export function ownedD2Resource(source) {
  const replacements = [
    ['async function Df(){', 'async function Df(owner){'],
    ['return new Worker(URL.createObjectURL(E),{type:"module"})', 'return owner?owner(E):new Worker(URL.createObjectURL(E),{type:"module"})'],
    ['constructor(){this.nextRequestId', 'constructor({workerFactory}={}){this.workerFactory=workerFactory;this.nextRequestId'],
    ['this.worker=await Df();', 'this.worker=await Df(this.workerFactory);if(this.disposed){this.worker.terminate();this.worker=void 0;throw Error("D2 instance has been disposed")}'],
    ['try{await this.ready}catch{}let E=this.worker;', 'let E=this.worker;'],
  ]
  for (const [before, after] of replacements) {
    if (source.split(before).length !== 2) throw Error(`D2 0.1.34 lifecycle adapter no longer matches: ${before}`)
    source = source.replace(before, after)
  }
  return '// @d2lang/d2 0.1.34, MPL-2.0. Modified by MarkNoria: injectable owned worker and immediate disposal.\n' + source
}
