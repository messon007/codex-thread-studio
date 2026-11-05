import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

test('fixed reader import rejects damaged, oversized and unsafe packages without replacing source', () => {
  execFileSync(process.platform === 'win32' ? 'python' : 'python3', [fileURLToPath(new URL('./import-marknoria-test.py', import.meta.url))], { stdio: 'pipe' })
})

test('vendored reader matches the externally reviewed package inventory', () => {
  execFileSync(process.platform === 'win32' ? 'python' : 'python3', [fileURLToPath(new URL('./import-marknoria.py', import.meta.url)), '--verify'], { stdio: 'pipe' })
})
