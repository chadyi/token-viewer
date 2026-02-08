import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useState } from 'react'

import type { UsageEntry } from '../types'

export function useUsageData() {
  const [data, setData] = useState<UsageEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const next = await invoke<UsageEntry[]>('scan_all_usage')
      setData(next)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return { data, loading, error, refresh }
}
