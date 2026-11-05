// Import a reviewed, hash-pinned public source/resource archive without patching it.
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
execFileSync(process.platform === 'win32' ? 'python' : 'python3', [fileURLToPath(new URL('./import-marknoria.py', import.meta.url)), ...process.argv.slice(2)], { stdio: 'inherit' })
