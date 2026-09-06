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

export function environmentSavePayload(input: {
  root: string
  configured: boolean
  secrets: string
  removeSecrets: string[]
  variables: string
  allowedHosts: string
  cacheVariables: string
  networkPolicy: 'restricted' | 'enabled'
}) {
  const secrets = parseEnvironmentLines(input.secrets, { allowEmpty: false })
  const removeSecrets = [...input.removeSecrets]
  for (const name of removeSecrets) {
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,127}$/u.test(name)) throw new Error(`Invalid secret name: ${name}`)
  }
  const variables = parseEnvironmentLines(input.variables, { allowEmpty: true })
  const allowedHosts = parseHosts(input.allowedHosts)
  const cacheVariables = parseEnvironmentLines(input.cacheVariables, { allowEmpty: false })
  const { networkPolicy, root } = input
  if (!input.configured && !Object.keys(variables).length && !Object.keys(secrets).length && !removeSecrets.length && !allowedHosts.length && !Object.keys(cacheVariables).length && networkPolicy === 'restricted') return null
  return { root, variables, secrets, removeSecrets, networkPolicy, allowedHosts, cacheVariables }
}
