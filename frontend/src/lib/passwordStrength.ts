// Rule-based password strength score 0..4. Cheap, no zxcvbn (170 kB).
// Each criterion adds 1 to the score; 12+ chars is a hard floor before any
// bar lights up.
const COMMON = new Set([
  'password',
  'qwerty',
  '123456',
  '12345678',
  'hunter2',
  'letmein',
  'welcome',
  'admin',
  'iloveyou',
  'monkey',
])

export function passwordStrength(pw: string): {
  score: 0 | 1 | 2 | 3 | 4
  label: 'too short' | 'weak' | 'fair' | 'good' | 'strong'
} {
  if (pw.length < 12) return { score: 0, label: 'too short' }
  if (COMMON.has(pw.toLowerCase())) return { score: 0, label: 'weak' }
  let s = 0
  if (/[a-z]/.test(pw)) s++
  if (/[A-Z]/.test(pw)) s++
  if (/\d/.test(pw)) s++
  if (/[^A-Za-z0-9]/.test(pw)) s++
  if (pw.length >= 16) s = Math.min(4, s + 1)
  const score = Math.min(4, s) as 0 | 1 | 2 | 3 | 4
  const label = (['weak', 'weak', 'fair', 'good', 'strong'] as const)[score]
  return { score, label }
}
