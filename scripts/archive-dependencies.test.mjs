import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createRequire } from 'node:module'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'
const require = createRequire(import.meta.url)

test('archive dependency overrides retain globbing, ZIP enumeration and extraction', async context => {
  const directory = await mkdtemp(join(tmpdir(), 'studio-archive-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const source = join(directory, 'source'); await mkdir(source)
  await writeFile(join(source, 'first.txt'), 'first')
  await writeFile(join(source, 'second.bin'), 'second')
  const glob = require('glob')
  assert.deepEqual(glob.sync('*.txt', { cwd: source }), ['first.txt'])
  assert.deepEqual(await glob.glob('*.txt', { cwd: source }), ['first.txt'])
  const archive = require('archiver')('zip'), chunks = []
  archive.on('data', chunk => chunks.push(chunk))
  const completed = new Promise((resolve, reject) => { archive.on('end', resolve); archive.on('error', reject) })
  archive.directory(source, 'directory')
  archive.glob('*.txt', { cwd: source }, { prefix: 'glob' })
  await archive.finalize(); await completed
  const buffer = Buffer.concat(chunks), unzipper = require('unzipper')
  const zip = await unzipper.Open.buffer(buffer)
  assert.deepEqual(zip.files.map(file => file.path).sort(), ['directory/first.txt', 'directory/second.bin', 'glob/first.txt'])
  const destination = join(directory, 'extracted')
  const extraction = unzipper.Extract({ path: destination }), extracted = extraction.promise()
  await pipeline(Readable.from([buffer]), extraction)
  await extracted
  assert.equal(await readFile(join(destination, 'glob/first.txt'), 'utf8'), 'first')
})

test('ExcelJS retains XLSX writing, ordinary reading and streamed reading', async context => {
  const directory = await mkdtemp(join(tmpdir(), 'studio-xlsx-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const ExcelJS = require('exceljs'), path = join(directory, 'book.xlsx')
  const workbook = new ExcelJS.Workbook(), sheet = workbook.addWorksheet('Values')
  sheet.addRows([['Name', 'Value'], ['first', 2], ['second', 3]])
  await workbook.xlsx.writeFile(path)
  const copy = new ExcelJS.Workbook(); await copy.xlsx.readFile(path)
  assert.equal(copy.getWorksheet('Values').getCell('B3').value, 3)
  // Use the streaming writer's inline-string format for its reader round trip.
  // The ordinary writer's ZIP entry ordering can vary by filesystem; ExcelJS's
  // streaming reader assumes workbook metadata precedes shared-string sheets.
  const streamPath = join(directory, 'streamed.xlsx')
  const streamed = new ExcelJS.stream.xlsx.WorkbookWriter({ filename: streamPath, useSharedStrings: false })
  const streamedSheet = streamed.addWorksheet('Values')
  for (const row of [['Name', 'Value'], ['first', 2], ['second', 3]]) streamedSheet.addRow(row).commit()
  await streamed.commit()
  const values = []
  const reader = new ExcelJS.stream.xlsx.WorkbookReader(streamPath, { sharedStrings: 'cache', worksheets: 'emit' })
  for await (const worksheet of reader) for await (const row of worksheet) values.push(row.getCell(2).value)
  assert.deepEqual(values, ['Value', 2, 3])
})
