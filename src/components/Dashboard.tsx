import {
  Area,
  AreaChart,
  CartesianGrid,
  Cell,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { useMemo } from 'react'
import type React from 'react'

import { useUsageData } from '../hooks/useUsageData'
import type { UsageEntry } from '../types'
import StatsCard from './StatsCard'
import UsageTable from './UsageTable'

const intFmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
const usdFmt = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  maximumFractionDigits: 6,
})

const darkTooltipStyle: React.CSSProperties = {
  backgroundColor: 'rgba(11, 14, 20, 0.95)',
  border: '1px solid rgba(255,255,255,0.12)',
  borderRadius: 10,
  padding: '10px 14px',
  color: 'rgba(255,255,255,0.92)',
  fontSize: 13,
}

type DailyPoint = {
  date: string
  tokens: number
  requests: number
  input: number
  output: number
  cost: number
}

function DailyTooltip({
  active,
  payload,
}: {
  active?: boolean
  payload?: Array<{
    payload: {
      date: string
      tokens: number
      requests: number
      input: number
      output: number
      cost: number
    }
  }>
}) {
  if (!active || !payload?.length) return null
  const d = payload[0].payload
  return (
    <div style={darkTooltipStyle}>
      <div style={{ fontWeight: 600, marginBottom: 6 }}>{d.date}</div>
      <div>Requests: {intFmt.format(d.requests)}</div>
      <div>Input Tokens: {intFmt.format(d.input)}</div>
      <div>Output Tokens: {intFmt.format(d.output)}</div>
      <div>Total Tokens: {intFmt.format(d.tokens)}</div>
      <div>Cost: {usdFmt.format(d.cost)}</div>
    </div>
  )
}

const TOOL_COLORS: Record<string, string> = {
  Claude: '#7bdff2',
  Codex: '#ffd37a',
  OpenCode: '#b4f8c8',
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

function sumTokens(e: UsageEntry): number {
  return (
    e.input_tokens +
    e.output_tokens +
    e.cache_read_tokens +
    e.cache_write_tokens
  )
}

export default function Dashboard() {
  const { data, loading, error, refresh } = useUsageData()

  const totals = useMemo(() => {
    let input = 0
    let output = 0
    let cost = 0
    let requests = 0
    for (const e of data) {
      input += e.input_tokens
      output += e.output_tokens
      cost += e.cost
      requests += 1
    }
    return { input, output, cost, requests }
  }, [data])

  const toolPie = useMemo(() => {
    const map = new Map<string, number>()
    for (const e of data) {
      map.set(e.tool, (map.get(e.tool) ?? 0) + sumTokens(e))
    }
    return [...map.entries()]
      .map(([name, value]) => ({ name, value }))
      .sort((a, b) => b.value - a.value)
  }, [data])

  const dailySeries = useMemo(() => {
    const map = new Map<string, DailyPoint>()
    for (const e of data) {
      const d = safeDate(e.timestamp)
      if (!d) continue
      const key = localDayKey(d)
      const cur = map.get(key) ?? {
        date: key,
        tokens: 0,
        requests: 0,
        input: 0,
        output: 0,
        cost: 0,
      }
      cur.tokens += sumTokens(e)
      cur.requests += 1
      cur.input += e.input_tokens
      cur.output += e.output_tokens
      cur.cost += e.cost
      map.set(key, cur)
    }
    return [...map.values()].sort((a, b) => a.date.localeCompare(b.date))
  }, [data])

  const dailyTokensFmt = useMemo(() => {
    let max = 0
    for (const s of dailySeries) max = Math.max(max, s.tokens)

    const absMax = Math.abs(max)
    let divisor = 1
    let suffix = ''
    if (absMax >= 1_000_000_000) {
      divisor = 1_000_000_000
      suffix = 'B'
    } else if (absMax >= 1_000_000) {
      divisor = 1_000_000
      suffix = 'M'
    } else if (absMax >= 1_000) {
      divisor = 1_000
      suffix = 'K'
    }

    return (v: unknown): string => {
      const n = typeof v === 'number' ? v : Number(v)
      if (!Number.isFinite(n)) return ''
      if (divisor === 1) return intFmt.format(n)
      return `${(n / divisor).toFixed(1)}${suffix}`
    }
  }, [dailySeries])

  return (
    <div>
      <div className="toolbar">
        <div>
          <div className="h1" style={{ margin: 0 }}>
            ccusage Viewer
          </div>
          <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
            {loading ? 'Scanning local logs...' : `Entries: ${intFmt.format(data.length)}`}
            {error ? ` · Error: ${error}` : null}
          </div>
        </div>
        <button className="btn" onClick={() => void refresh()} disabled={loading}>
          {loading ? 'Refreshing...' : 'Refresh'}
        </button>
      </div>

      <div className="grid cols-4">
        <StatsCard title="Total Requests" value={intFmt.format(totals.requests)} />
        <StatsCard title="Total Input Tokens" value={intFmt.format(totals.input)} />
        <StatsCard title="Total Output Tokens" value={intFmt.format(totals.output)} />
        <StatsCard title="Total Cost" value={usdFmt.format(totals.cost)} />
      </div>

      <div className="grid cols-2" style={{ marginTop: 12 }}>
        <div className="panel">
          <div className="h1">By Tool</div>
          <div style={{ height: 260 }}>
	            <ResponsiveContainer width="100%" height="100%">
	              <PieChart>
	                <Pie
	                  data={toolPie}
	                  dataKey="value"
	                  nameKey="name"
	                  outerRadius={90}
	                  label={({ value }) =>
	                    intFmt.format(typeof value === 'number' ? value : Number(value))
	                  }
	                >
	                  {toolPie.map((s, i) => (
	                    <Cell key={i} fill={TOOL_COLORS[s.name] ?? '#9aa7b6'} />
	                  ))}
	                </Pie>
	                <Tooltip
	                  contentStyle={{
	                    backgroundColor: 'rgba(11, 14, 20, 0.95)',
	                    border: '1px solid rgba(255,255,255,0.12)',
	                    borderRadius: 10,
	                  }}
	                  itemStyle={{ color: 'rgba(255,255,255,0.92)' }}
	                  labelStyle={{ color: 'rgba(255,255,255,0.92)' }}
	                  formatter={(v) => intFmt.format(typeof v === 'number' ? v : Number(v))}
	                />
	                <Legend />
	              </PieChart>
	            </ResponsiveContainer>
          </div>
        </div>

        <div className="panel">
          <div className="h1">Tokens By Day</div>
          <div style={{ height: 260 }}>
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={dailySeries}>
                <defs>
                  <linearGradient id="tokensFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#7bdff2" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#7bdff2" stopOpacity={0.02} />
                  </linearGradient>
                </defs>
                <CartesianGrid stroke="rgba(255,255,255,0.08)" vertical={false} />
                <XAxis
                  dataKey="date"
                  stroke="rgba(255,255,255,0.6)"
                  tick={{ fontSize: 12 }}
                />
                <YAxis
                  stroke="rgba(255,255,255,0.6)"
                  tick={{ fontSize: 12 }}
                  width={70}
                  tickFormatter={dailyTokensFmt}
                />
                <Tooltip content={<DailyTooltip />} />
                <Legend />
                <Area
                  type="monotone"
                  dataKey="tokens"
                  name="Tokens"
                  stroke="#7bdff2"
                  fill="url(#tokensFill)"
                  strokeWidth={2}
                  dot={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>

      <div style={{ marginTop: 12 }}>
        <UsageTable data={data} />
      </div>
    </div>
  )
}
