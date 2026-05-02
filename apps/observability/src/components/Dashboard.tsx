import { useEffect, useState } from "react";
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import {
  allowRatio,
  auditChainSeries,
  denyRatio,
  latencyPercentiles,
  loadMetrics,
  requestsPerSecond,
  shortTimeLabel,
  type MetricsSnapshot,
} from "../lib/metrics.js";
import { MOCK_METRICS } from "../data/mock-metrics.js";

interface DashboardProps {
  /** If set, dashboard fetches `<endpoint>/v1/admin/metrics` on mount. Otherwise uses MOCK_METRICS. */
  endpoint?: string | undefined;
  /** Override the initial snapshot — used by tests. */
  initialSnapshot?: MetricsSnapshot;
}

const PANEL_HEIGHT = 280;

const AXIS_PROPS = {
  stroke: "#7c8694",
  fontSize: 11,
} as const;

const GRID_PROPS = {
  stroke: "#1d2230",
  strokeDasharray: "3 3",
} as const;

export function Dashboard({ endpoint, initialSnapshot }: DashboardProps) {
  const [snap, setSnap] = useState<MetricsSnapshot>(initialSnapshot ?? MOCK_METRICS);
  const [source, setSource] = useState<"mock" | "live">(
    initialSnapshot !== undefined ? "live" : "mock",
  );
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    if (initialSnapshot !== undefined) return;
    // Resolve the effective endpoint at hydration time. Prop wins; otherwise
    // read `?endpoint=...` from the URL so users can flip mock→live without
    // a build-time rebuild (the static site is one HTML file).
    let resolved = endpoint;
    if (resolved === undefined && typeof window !== "undefined") {
      const fromQuery = new URL(window.location.href).searchParams.get("endpoint");
      if (fromQuery !== null && fromQuery.length > 0) resolved = fromQuery;
    }
    if (resolved === undefined) return;
    let cancelled = false;
    void (async () => {
      const live = await loadMetrics(resolved!);
      if (cancelled) return;
      if (live === undefined) {
        setError(
          `Could not load /v1/admin/metrics from ${resolved}. Showing mock data.`,
        );
        return;
      }
      setSnap(live);
      setSource("live");
    })();
    return () => {
      cancelled = true;
    };
  }, [endpoint, initialSnapshot]);

  const rps = requestsPerSecond(snap).map((d) => ({
    ...d,
    label: shortTimeLabel(d.ts),
  }));
  const allow = allowRatio(snap).map((d) => ({ ...d, label: shortTimeLabel(d.ts) }));
  const deny = denyRatio(snap).map((d) => ({ ...d, label: shortTimeLabel(d.ts) }));
  const allowDeny = allow.map((a, i) => ({
    label: a.label,
    allow_pct: a.pct,
    deny_pct: deny[i]?.pct ?? 0,
  }));
  const audit = auditChainSeries(snap).map((d) => ({
    ...d,
    label: shortTimeLabel(d.ts),
  }));
  const latency = latencyPercentiles(snap).map((d) => ({
    ...d,
    label: shortTimeLabel(d.ts),
  }));

  return (
    <div className="dashboard">
      <header className="banner">
        <div>
          <span className={`source-pill source-${source}`}>{source.toUpperCase()}</span>
          <span className="endpoint">{snap.daemon.endpoint}</span>
        </div>
        {snap.daemon.version !== undefined && (
          <div className="meta">daemon v{snap.daemon.version}</div>
        )}
      </header>

      {error !== undefined && <div className="error-banner">{error}</div>}

      <div className="panels">
        <Panel title="Requests / sec" subtitle="last 60 minutes, 1-minute buckets">
          <ResponsiveContainer width="100%" height={PANEL_HEIGHT}>
            <LineChart data={rps}>
              <CartesianGrid {...GRID_PROPS} />
              <XAxis dataKey="label" {...AXIS_PROPS} />
              <YAxis {...AXIS_PROPS} />
              <Tooltip
                contentStyle={{ background: "#0e1118", border: "1px solid #2a3142" }}
              />
              <Line
                type="monotone"
                dataKey="rps"
                stroke="#5eb3ff"
                strokeWidth={2}
                dot={false}
                name="req/s"
              />
            </LineChart>
          </ResponsiveContainer>
        </Panel>

        <Panel title="Allow vs Deny" subtitle="% of requests per bucket">
          <ResponsiveContainer width="100%" height={PANEL_HEIGHT}>
            <LineChart data={allowDeny}>
              <CartesianGrid {...GRID_PROPS} />
              <XAxis dataKey="label" {...AXIS_PROPS} />
              <YAxis domain={[0, 100]} {...AXIS_PROPS} />
              <Tooltip
                contentStyle={{ background: "#0e1118", border: "1px solid #2a3142" }}
              />
              <Legend wrapperStyle={{ fontSize: 12 }} />
              <Line
                type="monotone"
                dataKey="allow_pct"
                stroke="#4ade80"
                strokeWidth={2}
                dot={false}
                name="allow %"
              />
              <Line
                type="monotone"
                dataKey="deny_pct"
                stroke="#f87171"
                strokeWidth={2}
                dot={false}
                name="deny + requires_human %"
              />
            </LineChart>
          </ResponsiveContainer>
        </Panel>

        <Panel title="Audit-chain growth" subtitle="cumulative events appended">
          <ResponsiveContainer width="100%" height={PANEL_HEIGHT}>
            <LineChart data={audit}>
              <CartesianGrid {...GRID_PROPS} />
              <XAxis dataKey="label" {...AXIS_PROPS} />
              <YAxis tickFormatter={(v) => v.toLocaleString()} {...AXIS_PROPS} />
              <Tooltip
                contentStyle={{ background: "#0e1118", border: "1px solid #2a3142" }}
                formatter={(value: number) => value.toLocaleString()}
              />
              <Line
                type="monotone"
                dataKey="length"
                stroke="#fbbf24"
                strokeWidth={2}
                dot={false}
                name="audit_chain_length"
              />
            </LineChart>
          </ResponsiveContainer>
        </Panel>

        <Panel title="Decision latency" subtitle="p50 / p95 / p99 (ms)">
          <ResponsiveContainer width="100%" height={PANEL_HEIGHT}>
            <LineChart data={latency}>
              <CartesianGrid {...GRID_PROPS} />
              <XAxis dataKey="label" {...AXIS_PROPS} />
              <YAxis {...AXIS_PROPS} />
              <Tooltip
                contentStyle={{ background: "#0e1118", border: "1px solid #2a3142" }}
              />
              <Legend wrapperStyle={{ fontSize: 12 }} />
              <Line
                type="monotone"
                dataKey="p50"
                stroke="#5eb3ff"
                strokeWidth={2}
                dot={false}
                name="p50"
              />
              <Line
                type="monotone"
                dataKey="p95"
                stroke="#a78bfa"
                strokeWidth={2}
                dot={false}
                name="p95"
              />
              <Line
                type="monotone"
                dataKey="p99"
                stroke="#fb923c"
                strokeWidth={2}
                dot={false}
                name="p99"
              />
            </LineChart>
          </ResponsiveContainer>
        </Panel>
      </div>
    </div>
  );
}

interface PanelProps {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}

function Panel({ title, subtitle, children }: PanelProps) {
  return (
    <section className="panel">
      <div className="panel-head">
        <h2>{title}</h2>
        <span className="subtitle">{subtitle}</span>
      </div>
      {children}
    </section>
  );
}
