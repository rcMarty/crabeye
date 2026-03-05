export function formatDateEU(value: string | Date | null | undefined): string {
  if (!value) return ''

  if (typeof value === 'string') {
    const euMatch = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(value.trim())
    if (euMatch) {
      return value.trim()
    }

    const isoMatch = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim())
    if (isoMatch) {
      return `${isoMatch[3]}/${isoMatch[2]}/${isoMatch[1]}`
    }

    const parsed = new Date(value)
    if (!Number.isNaN(parsed.getTime())) {
      return parsed.toLocaleDateString('en-GB')
    }

    return ''
  }

  return value.toLocaleDateString('en-GB')
}

export function toIsoDate(value: string | null | undefined): string | null {
  if (!value) return null
  const trimmed = value.trim()

  const euMatch = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(trimmed)
  if (euMatch) {
    return `${euMatch[3]}-${euMatch[2]}-${euMatch[1]}`
  }

  const isoMatch = /^(\d{4})-(\d{2})-(\d{2})$/.exec(trimmed)
  if (isoMatch) {
    return trimmed
  }

  return null
}

export function formatDateTimeEU(value: string | Date | null | undefined): string {
  if (!value) return ''
  const parsed = typeof value === 'string' ? new Date(value) : value
  if (!parsed || Number.isNaN(parsed.getTime())) return ''
  return parsed.toLocaleString('en-GB')
}
