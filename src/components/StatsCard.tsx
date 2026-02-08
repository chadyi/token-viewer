type Props = {
  title: string
  value: string
  subtitle?: string
}

export default function StatsCard({ title, value, subtitle }: Props) {
  return (
    <div className="panel" style={{ padding: 14 }}>
      <div className="muted" style={{ fontSize: 12, letterSpacing: 0.2 }}>
        {title}
      </div>
      <div style={{ fontSize: 22, fontWeight: 650, marginTop: 6 }}>{value}</div>
      {subtitle ? (
        <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>
          {subtitle}
        </div>
      ) : null}
    </div>
  )
}
