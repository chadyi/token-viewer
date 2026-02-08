import { Fragment, useEffect, useMemo, useState } from 'react'
import { ChevronRight, ChevronDown } from 'lucide-react'

import type { UsageEntry } from '../types'

const numFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
const usdFmt = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  maximumFractionDigits: 6,
})

type Granularity = 'day' | 'week' | 'month'

type DateRow = {
  date: string
  requests: number
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
  cost: number
}

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

function safeDate(ts: string): Date | null {
  const d = new Date(ts)
  return Number.isFinite(d.getTime()) ? d : null
}

function localDayKey(d: Date): string {
  const yyyy = d.getFullYear()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  return `${yyyy}-${mm}-${dd}`
}

function localMonthKey(d: Date): string {
  const yyyy = d.getFullYear()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  return `${yyyy}-${mm}`
}

function isoWeekKey(d: Date): string {
  // Use local date for grouping, but compute ISO week-year/week number.
  const date = new Date(d.getTime())
  date.setHours(0, 0, 0, 0)

  // ISO: Monday=1..Sunday=7; JS: Sunday=0..Saturday=6.
  const day = date.getDay() || 7
  date.setDate(date.getDate() + 4 - day) // nearest Thursday decides ISO year

  const isoYear = date.getFullYear()
  const yearStart = new Date(isoYear, 0, 1)
  yearStart.setHours(0, 0, 0, 0)

  const diffDays = Math.floor((date.getTime() - yearStart.getTime()) / 86400000) + 1
  const week = Math.ceil(diffDays / 7)

  return `${isoYear}-W${String(week).padStart(2, '0')}`
}

function totalTokens(r: Pick<ModelRow, 'input_tokens' | 'output_tokens' | 'cache_read_tokens' | 'cache_write_tokens'>): number {
  return r.input_tokens + r.output_tokens + r.cache_read_tokens + r.cache_write_tokens
}

export default function DateTable({ data }: Props) {
  const [granularity, setGranularity] = useState<Granularity>('day')
  const [expandedKey, setExpandedKey] = useState<string | null>(null)

  // Changing granularity changes the meaning of "date key"; collapse any expanded row to avoid mismatch.
  useEffect(() => {
    setExpandedKey(null)
  }, [granularity])

  const { rows, entriesByKey } = useMemo(() => {
    const rowMap = new Map<string, DateRow>()
    const entryMap = new Map<string, UsageEntry[]>()

    for (const e of data) {
      const d = safeDate(e.timestamp)
      if (!d) continue

      const key =
        granularity === 'day'
          ? localDayKey(d)
          : granularity === 'week'
            ? isoWeekKey(d)
            : localMonthKey(d)

      const cur =
        rowMap.get(key) ??
        ({
          date: key,
          requests: 0,
          input_tokens: 0,
          output_tokens: 0,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          cost: 0,
        } satisfies DateRow)

      cur.requests += 1
      cur.input_tokens += e.input_tokens
      cur.output_tokens += e.output_tokens
      cur.cache_read_tokens += e.cache_read_tokens
      cur.cache_write_tokens += e.cache_write_tokens
      cur.cost += e.cost
      rowMap.set(key, cur)

      const arr = entryMap.get(key)
      if (arr) arr.push(e)
      else entryMap.set(key, [e])
    }

    const rows = [...rowMap.values()].sort((a, b) => b.date.localeCompare(a.date))
    return { rows, entriesByKey: entryMap }
  }, [data, granularity])

  const expandedModelRows = useMemo<ModelRow[]>(() => {
    if (!expandedKey) return []
    const entries = entriesByKey.get(expandedKey)
    if (!entries?.length) return []

    const map = new Map<string, ModelRow>()
    for (const e of entries) {
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
  }, [entriesByKey, expandedKey])

  return (
    <div className="panel">
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 12,
          marginBottom: 12,
        }}
      >
        <div className="h1" style={{ margin: 0 }}>
          By Date
        </div>
        <div className="btn-group">
          <button
            className={`btn${granularity === 'day' ? ' active' : ''}`}
            onClick={() => setGranularity('day')}
          >
            Day
          </button>
          <button
            className={`btn${granularity === 'week' ? ' active' : ''}`}
            onClick={() => setGranularity('week')}
          >
            Week
          </button>
          <button
            className={`btn${granularity === 'month' ? ' active' : ''}`}
            onClick={() => setGranularity('month')}
          >
            Month
          </button>
        </div>
      </div>

      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr>
              {['Date', 'Requests', 'Input', 'Output', 'Cache Read', 'Cache Write', 'Total', 'Cost'].map(
                (h) => (
                  <th
                    key={h}
                    className="muted"
                    style={{
                      textAlign: h === 'Date' ? 'left' : 'right',
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
                <td className="muted" colSpan={8} style={{ padding: 12, textAlign: 'center' }}>
                  No data
                </td>
              </tr>
            ) : (
              rows.map((r) => {
                const isExpanded = expandedKey === r.date
                return (
                  <Fragment key={r.date}>
                    <tr
                      onClick={() => setExpandedKey((cur) => (cur === r.date ? null : r.date))}
                      style={{ cursor: 'pointer' }}
                    >
                      <td style={{ padding: '10px 8px', textAlign: 'left', whiteSpace: 'nowrap' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <span
                            className="muted"
                            style={{
                              width: 14,
                              display: 'inline-flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                            }}
                          >
                            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                          </span>
                          <span>{r.date}</span>
                        </div>
                      </td>
                      <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                        {numFmt.format(r.requests)}
                      </td>
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
                      <td style={{ padding: '10px 8px', textAlign: 'right' }}>
                        {numFmt.format(totalTokens(r))}
                      </td>
                      <td style={{ padding: '10px 8px', textAlign: 'right', whiteSpace: 'nowrap' }}>
                        {usdFmt.format(r.cost)}
                      </td>
                    </tr>

                    {isExpanded ? (
                      <tr>
                        <td colSpan={8} style={{ padding: 0 }}>
                          <div
                            style={{
                              background: 'rgba(255,255,255,0.03)',
                              padding: '10px 8px 12px 28px',
                            }}
                          >
                            <div style={{ overflowX: 'auto' }}>
                              <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
                                <thead>
                                  <tr>
                                    {[
                                      'Model',
                                      'Requests',
                                      'Input',
                                      'Output',
                                      'Cache Read',
                                      'Cache Write',
                                      'Total',
                                      'Cost',
                                    ].map((h) => (
                                      <th
                                        key={h}
                                        className="muted"
                                        style={{
                                          textAlign: h === 'Model' ? 'left' : 'right',
                                          fontWeight: 600,
                                          padding: '8px 8px',
                                          borderBottom: '1px solid rgba(255,255,255,0.10)',
                                          whiteSpace: 'nowrap',
                                        }}
                                      >
                                        {h}
                                      </th>
                                    ))}
                                  </tr>
                                </thead>
                                <tbody>
                                  {expandedModelRows.length === 0 ? (
                                    <tr>
                                      <td
                                        className="muted"
                                        colSpan={8}
                                        style={{ padding: 12, textAlign: 'center' }}
                                      >
                                        No data
                                      </td>
                                    </tr>
                                  ) : (
                                    expandedModelRows.map((mr, idx) => (
                                      <tr key={`${mr.model}:${idx}`}>
                                        <td
                                          style={{
                                            padding: '8px 8px',
                                            textAlign: 'left',
                                            whiteSpace: 'nowrap',
                                          }}
                                        >
                                          {mr.model}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(mr.requests)}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(mr.input_tokens)}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(mr.output_tokens)}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(mr.cache_read_tokens)}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(mr.cache_write_tokens)}
                                        </td>
                                        <td style={{ padding: '8px 8px', textAlign: 'right' }}>
                                          {numFmt.format(totalTokens(mr))}
                                        </td>
                                        <td
                                          style={{
                                            padding: '8px 8px',
                                            textAlign: 'right',
                                            whiteSpace: 'nowrap',
                                          }}
                                        >
                                          {usdFmt.format(mr.cost)}
                                        </td>
                                      </tr>
                                    ))
                                  )}
                                </tbody>
                              </table>
                            </div>
                          </div>
                        </td>
                      </tr>
                    ) : null}
                  </Fragment>
                )
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
