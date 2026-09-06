export function parseEnvironmentLines(value: unknown, { allowEmpty = false } = {}) {
  const output: Record<string, string> = {}
  for (const raw of String(value || '').split(/\r?\n/gu)) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const split = line.indexOf('=')
    const name = (split < 0 ? line : line.slice(0, split)).trim()
    const content = split < 0 ? '' : line.slice(split + 1)
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,127}$/u.test(name)) throw new Error(`Invalid environment variable: ${name}`)
    if (!allowEmpty && !content) throw new Error(`${name} requires a value`)
    output[name] = content
  }
  return output
}

export function formatEnvironmentLines(values: Record<string, unknown> = {}) {
  return Object.entries(values).map(([name, value]) => `${name}=${value}`).join('\n')
}

export function parseHosts(value: unknown) {
  return [...new Set(String(value || '').split(/[\s,]+/gu).map((host) => host.trim().toLowerCase()).filter(Boolean))]
}
