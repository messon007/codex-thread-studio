import test from 'node:test'
import assert from 'node:assert/strict'
import { pluginReferences, composerReferenceInput, matchingSkills, selectedSkillReference } from './composer-tools.mjs'

test('plugin picker includes only installed enabled plugins with marketplace identity', () => {
  const active = { id: 'sample@market', name: 'sample', installed: true, enabled: true, interface: { displayName: 'Example Plugin', shortDescription: 'Read documents' } }
  const refs = pluginReferences({ marketplaces: [{ name: 'market', plugins: [active, active,
    { ...active, id: 'disabled', enabled: false }, { ...active, id: 'uninstalled', installed: false }] }] })
  assert.equal(refs.length, 1)
  assert.equal(matchingSkills('example', refs).length, 1)
  assert.equal(matchingSkills('market', refs).length, 1)
  assert.equal(selectedSkillReference(refs[0]), '$sample ')
  assert.deepEqual(composerReferenceInput(refs[0]), { type: 'mention', name: 'sample', path: 'plugin://sample@market' })
  assert.deepEqual(composerReferenceInput({ name: 'local', path: '/skills/local/SKILL.md' }), { type: 'skill', name: 'local', path: '/skills/local/SKILL.md' })
  assert.equal(composerReferenceInput({ name: 'missing' }), null)
})
