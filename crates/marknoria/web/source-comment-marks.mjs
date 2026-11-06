const mapped = new WeakMap()
// Scan an immutable bounded source window once for all requested endpoints.
// Original UTF-8 bytes and CRLF stay distinct from CodeMirror's UTF-16/LF.
function endpoints(page, positions) {
  const sorted = [...new Set(positions)].sort((a,b)=>a-b), key=sorted.join(',')
  const previous=mapped.get(page)
  if(previous?.key===key)return previous.result
  const result=new Map();let target=0,bytes=page.start,units=0
  for(let offset=0;offset<=page.text.length;){
    while(sorted[target]<bytes)target++ // A split UTF-8/CRLF boundary is invalid.
    while(sorted[target]===bytes){result.set(bytes,units);target++}
    if(target>=sorted.length||offset===page.text.length)break
    const scalar=page.text.codePointAt(offset),width=scalar>65535?2:1
    if(scalar===13&&page.text[offset+1]==='\n'){bytes+=2;units++;offset+=2}
    else {bytes+=scalar<128?1:scalar<2048?2:scalar<65536?3:4;units+=width;offset+=width}
  }
  mapped.set(page,{key,result});return result
}
export function sourceCommentContext(code, page) {
  if (!code || !page || page.end - page.start > 512 * 1024) return { scopes: [] }
  const view = code.view, scopes = []
  for (let bucket = Math.floor(page.start / 16384); bucket <= Math.floor(Math.max(page.start, page.end - 1) / 16384); bucket++) scopes.push({ key: `s:${bucket}` })
  return {
    scopes,
    viewport: view.scrollDOM,
    resolve(notes) {
      const shapes = [], selections=[]
      for (const note of notes) {
        const location = note.anchor?.location
        if (location?.kind !== 'source') continue
        const start = Math.max(page.start, location.startByte), end = Math.min(page.end, location.endByte)
        if (start < end) selections.push({note,start,end})
      }
      const offsets=endpoints(page,selections.flatMap(value=>[value.start,value.end]))
      for(const {note,start,end} of selections){
        if(!offsets.has(start)||!offsets.has(end))continue
        for (const visible of view.visibleRanges) {
          const from = Math.max(offsets.get(start), visible.from), to = Math.min(offsets.get(end), visible.to)
          if (to <= from) continue
          const a = view.domAtPos(from, 1), b = view.domAtPos(to, -1), range = view.dom.ownerDocument.createRange()
          range.setStart(a.node, a.offset); range.setEnd(b.node, b.offset)
          shapes.push({ id: note.id, range, clip: view.scrollDOM })
          if (shapes.length > 512) throw Error('Source comment geometry exceeds its budget')
        }
      }
      return shapes
    }
  }
}
