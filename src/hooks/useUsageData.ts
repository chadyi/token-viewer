import { invoke } from '@tauri-apps/api/core'
import { useCallback, useEffect, useRef, useState } from 'react'

import type { UsageEntry } from '../types'

export function useUsageData() {
  const [data, setData] = useState<UsageEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const hasFullLoaded = useRef(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const cmd = hasFullLoaded.current
        ? 'scan_all_usage_incremental'
        : 'scan_all_usage'
      const next = await invoke<UsageEntry[]>(cmd)
      setData(next)
      hasFullLoaded.current = true
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
