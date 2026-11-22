export interface TrendPoint {
  timestamp: number
  value: number
}

const TREND_SAMPLES = 120
const TREND_BANDWIDTH_SHARE_OF_SPAN = 1 / 12
const TREND_MAX_BANDWIDTH_SECONDS = 90 * 86_400

function interpolate(points: readonly [number, number][], t: number): number {
  const index = points.findIndex(([time]) => time >= t)
  if (index === -1) return points[points.length - 1]![1]
  if (index === 0) return points[0]![1]
  const [t0, v0] = points[index - 1]!
  const [t1, v1] = points[index]!
  return t1 === t0 ? v1 : v0 + ((v1 - v0) * (t - t0)) / (t1 - t0)
}

// Same algorithm and constants as `value_trend` in server/app-core/src/api/properties.rs, so
// every client draws the same line.
export function valueTrend(
  valuations: readonly TrendPoint[],
  now: number,
  currentValue: number | null
): TrendPoint[] {
  const points = valuations
    .filter((point) => point.timestamp <= now)
    .map((point): [number, number] => [point.timestamp, point.value])
    .sort((a, b) => a[0] - b[0])
  const last = points[points.length - 1]
  if (last === undefined) return []
  points.push([now, currentValue ?? last[1]])

  const start = points[0]![0]
  const span = now - start
  if (span <= 0) {
    return [{ timestamp: now, value: points[points.length - 1]![1] }]
  }

  const times = Array.from(
    { length: TREND_SAMPLES },
    (_, k) => start + (span * k) / (TREND_SAMPLES - 1)
  )
  const linear = times.map((t) => interpolate(points, t))
  const sigma = Math.min(
    span * TREND_BANDWIDTH_SHARE_OF_SPAN,
    TREND_MAX_BANDWIDTH_SECONDS
  )
  const smooth = times.map((t) => {
    let weighted = 0
    let total = 0
    times.forEach((u, j) => {
      const weight = Math.exp(-(((t - u) / sigma) ** 2) / 2)
      weighted += weight * linear[j]!
      total += weight
    })
    return weighted / total
  })

  const firstGap = linear[0]! - smooth[0]!
  const lastGap = linear[TREND_SAMPLES - 1]! - smooth[TREND_SAMPLES - 1]!
  return times.map((t, k) => ({
    timestamp: Math.round(t),
    value:
      smooth[k]! + firstGap + ((lastGap - firstGap) * k) / (TREND_SAMPLES - 1),
  }))
}
