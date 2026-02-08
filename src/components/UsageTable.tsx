import { useMemo } from 'react'

import type { UsageEntry } from '../types'

const numFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
const usdFmt = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  maximumFractionDigits: 6,
})

type ModelRow = {
  model: string
  requests: number
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  cost: number
}

type Props = {
  data: UsageEntry[]
}

function totalTokens(r: ModelRow): number {
  return r.input_tokens + r.output_tokens + r.cache_read_tokens + r.cache_write_tokens
}

export default function UsageTable({ data }: Props) {
  const rows = useMemo<ModelRow[]>(() => {
    const map = new Map<string, ModelRow>()
    for (const e of data) {
      const cur =
        map.get(e.model) ??
        ({
          model: e.model,
          requests: 0,
          input_tokens: 0,
          output_tokens: 0,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          cost: 0,
        } satisfies ModelRow)

      cur.requests += 1
      cur.input_tokens += e.input_tokens
      cur.output_tokens += e.output_tokens
      cur.cache_read_tokens += e.cache_read_tokens
      cur.cache_write_tokens += e.cache_write_tokens
      cur.cost += e.cost
      map.set(e.model, cur)
    }

    return [...map.values()].sort((a, b) => {
      const costDiff = b.cost - a.cost
      if (costDiff !== 0) return costDiff
      return totalTokens(b) - totalTokens(a)
    })
  }, [data])

  return (
    <div className="panel">
      <div className="h1">By Model</div>
      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr>
              {['Model', 'Requests', 'Input', 'Output', 'Cache Read', 'Cache Write', 'Cost'].map(
                (h) => (
                  <th
                    key={h}
                    className="muted"
                    style={{
                      textAlign: h === 'Model' ? 'left' : 'right',
                      fontWeight: 600,
                      padding: '10px 8px',
                      borderBottom: '1px solid rgba(255,255,255,0.12)',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {h}
                  </th>
                ),
              )}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td className="muted" colSpan={7} style={{ padding: 12, textAlign: 'center' }}>
                  No data
                </td>
              </tr>
            ) : (
              rows.map((r, idx) => (
                <tr key={`${r.model}:${idx}`}>
                  <td style={{ padding: '10px 8px', textAlign: 'left', whiteSpace: 'nowrap' }}>
                    {r.model}
                  </td>
                  <td style={{ padding: '10px 8px', textAlign: 'right' }}>{numFmt.format(r.requests)}</td>
                  <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                    {numFmt.format(r.input_tokens)}
                  </td>
                  <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                    {numFmt.format(r.output_tokens)}
                  </td>
                  <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                    {numFmt.format(r.cache_read_tokens)}
                  </td>
                  <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                    {numFmt.format(r.cache_write_tokens)}
                  </td>
                  <td style={{ padding: '10px 8px', textAlign: 'right', whiteSpace: 'nowrap' }}>
                    {usdFmt.format(r.cost)}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
