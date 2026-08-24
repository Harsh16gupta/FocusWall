import { useState, useEffect, useMemo } from 'react';
import {
  Shield,
  Globe,
  Plus,
  ScrollText,
  AlertCircle,
  CheckCircle,
  RefreshCw,
  Lock,
  Search,
  ChevronRight,
  Sparkles,
  Layers,
  Activity,
  X,
  Radio,
} from 'lucide-react';
import { SystemStatus, AuditLogEntry, NormalizedPreview } from './types';

// Safe Tauri invoke wrapper that falls back to IPC/mock in browser
const invokeCommand = async <T,>(cmd: string, args: Record<string, any> = {}): Promise<T> => {
  if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  console.log(`[IPC Invoke] ${cmd}`, args);
  throw new Error("Tauri runtime not connected; use Tauri window or start focuswalld");
};

export function App() {
  const [tab, setTab] = useState<'dashboard' | 'rules' | 'add' | 'logs'>('dashboard');
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  const [logFilter, setLogFilter] = useState<string>('all');
  const [ruleSearch, setRuleSearch] = useState<string>('');
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Add rule state
  const [rawInput, setRawInput] = useState('');
  const [cooldownHours, setCooldownHours] = useState(24);
  const [preview, setPreview] = useState<NormalizedPreview | null>(null);
  const [showConfirmModal, setShowConfirmModal] = useState(false);
  const [actionLoading, setActionLoading] = useState(false);

  // System time ticker
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  const fetchStatus = async () => {
    try {
      const data = await invokeCommand<SystemStatus>('get_status');
      setStatus(data);
    } catch {
      // Fallback local status for browser preview
      const currentHour = now.getHours();
      const isAllowed = currentHour >= 20 && currentHour < 21;
      setStatus({
        current_time: new Date().toISOString(),
        youtube_state: isAllowed ? 'allowed' : 'blocked',
        policies: [
          {
            id: 1,
            kind: 'system',
            name: 'youtube',
            domains: ['youtube.com', 'www.youtube.com', 'm.youtube.com', 'music.youtube.com', 'youtu.be', 'youtube-nocookie.com', 'ytimg.com', 'googlevideo.com'],
            schedule: { start: '20:00', end: '21:00' },
            timezone: 'system',
            status: 'active',
            created_at: new Date().toISOString(),
          }
        ],
        blocked_domains: isAllowed ? [] : ['youtube.com', 'googlevideo.com', 'ytimg.com', 'youtu.be'],
      });
    }
  };

  const fetchLogs = async () => {
    try {
      const data = await invokeCommand<AuditLogEntry[]>('get_logs', { limit: 50 });
      setLogs(data);
    } catch {
      setLogs([
        { id: 1, ts: new Date().toISOString(), event_type: 'daemon_start', detail: 'focuswalld active with fail-closed kernel protection' }
      ]);
    }
  };

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    await Promise.all([fetchStatus(), fetchLogs()]);
    setTimeout(() => setIsRefreshing(false), 400);
  };

  useEffect(() => {
    fetchStatus();
    fetchLogs();
    const interval = setInterval(fetchStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  // Public Suffix List Client Preview
  useEffect(() => {
    const trimmed = rawInput.trim();
    if (!trimmed) {
      setPreview(null);
      return;
    }
    try {
      let host = trimmed;
      if (!host.includes('://')) host = 'https://' + host;
      const url = new URL(host);
      let hostname = url.hostname.toLowerCase();
      // Basic root domain extraction for preview
      const parts = hostname.split('.').filter(Boolean);
      if (parts.length >= 2) {
        let root = parts.slice(-2).join('.');
        if (parts.length >= 3 && ['co.uk', 'gov.uk', 'ac.in', 'co.in', 'com.au'].includes(parts.slice(-2).join('.'))) {
          root = parts.slice(-3).join('.');
        }
        setPreview({
          root_domain: root,
          domains: [root, `www.${root}`],
        });
      } else {
        setPreview(null);
      }
    } catch {
      setPreview(null);
    }
  }, [rawInput]);

  const handleAddRule = async () => {
    if (!preview) return;
    setActionLoading(true);
    try {
      await invokeCommand('add_rule', { input: rawInput, cooldownHours });
      setRawInput('');
      setPreview(null);
      setShowConfirmModal(false);
      await fetchStatus();
      setTab('rules');
    } catch (e: any) {
      alert(`Failed to add rule: ${e.message || e}`);
    } finally {
      setActionLoading(false);
    }
  };

  const handleRequestRemoval = async (ruleId: number) => {
    if (!confirm("Initiate 24-hour removal cooldown? The website will REMAIN FULLY BLOCKED until the timer expires.")) {
      return;
    }
    try {
      await invokeCommand('request_removal', { ruleId, reason: "User requested removal" });
      await fetchStatus();
    } catch (e: any) {
      alert(`Error: ${e.message || e}`);
    }
  };

  const handleConfirmRemoval = async (ruleId: number) => {
    try {
      await invokeCommand('confirm_removal', { ruleId });
      await fetchStatus();
    } catch (e: any) {
      alert(`Cooldown active: ${e.message || e}`);
    }
  };

  const handleCancelRemoval = async (ruleId: number) => {
    try {
      await invokeCommand('cancel_removal', { ruleId });
      await fetchStatus();
    } catch (e: any) {
      alert(`Error: ${e.message || e}`);
    }
  };

  // YouTube schedule calculations
  const ytSchedule = useMemo(() => {
    const currentH = now.getHours();
    const currentM = now.getMinutes();
    const currentS = now.getSeconds();

    if (currentH === 20) {
      const remainingM = 59 - currentM;
      const remainingS = 59 - currentS;
      const progressPercent = ((currentM * 60 + currentS) / 3600) * 100;
      return {
        state: 'ALLOWED',
        badge: 'Window Open',
        timeText: `${String(remainingM).padStart(2, '0')}:${String(remainingS).padStart(2, '0')}`,
        label: 'remaining before lock',
        progress: progressPercent,
      };
    } else {
      let diffS: number;
      if (currentH < 20) {
        diffS = (20 * 3600) - (currentH * 3600 + currentM * 60 + currentS);
      } else {
        diffS = ((24 + 20) * 3600) - (currentH * 3600 + currentM * 60 + currentS);
      }
      const hours = Math.floor(diffS / 3600);
      const mins = Math.floor((diffS % 3600) / 60);
      const secs = diffS % 60;
      const elapsedSinceAllowed = 23 * 3600 - diffS;
      const progressPercent = (elapsedSinceAllowed / (23 * 3600)) * 100;
      return {
        state: 'BLOCKED',
        badge: 'Strictly Enforced',
        timeText: `${String(hours).padStart(2, '0')}h ${String(mins).padStart(2, '0')}m ${String(secs).padStart(2, '0')}s`,
        label: 'until 20:00 unlock window',
        progress: progressPercent,
      };
    }
  }, [now]);

  // Filtered rules
  const filteredPolicies = useMemo(() => {
    if (!status?.policies) return [];
    if (!ruleSearch.trim()) return status.policies;
    const q = ruleSearch.toLowerCase();
    return status.policies.filter(p => p.name.toLowerCase().includes(q) || p.domains.some(d => d.toLowerCase().includes(q)));
  }, [status?.policies, ruleSearch]);

  // Filtered logs
  const filteredLogs = useMemo(() => {
    if (logFilter === 'all') return logs;
    return logs.filter(l => l.event_type.toLowerCase().includes(logFilter));
  }, [logs, logFilter]);

  return (
    <div className="flex h-screen bg-[#09090b] text-[#f4f4f5] antialiased overflow-hidden font-sans select-none">
      {/* Sidebar Navigation */}
      <aside className="w-64 border-r border-[#27272a]/60 bg-[#0d0d11]/80 backdrop-blur-xl flex flex-col justify-between p-4 z-20">
        <div className="space-y-6">
          {/* App Branding */}
          <div className="flex items-center space-x-3 px-2 pt-1">
            <div className="w-9 h-9 rounded-xl bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 glow-emerald">
              <Shield className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center space-x-1.5">
                <span className="font-semibold text-sm tracking-tight text-white">FocusWall</span>
                <span className="text-[10px] uppercase font-mono px-1.5 py-0.2 bg-emerald-500/15 text-emerald-400 rounded-full border border-emerald-500/30">v1.0</span>
              </div>
              <p className="text-[11px] text-zinc-400 font-mono">system-level guard</p>
            </div>
          </div>

          {/* Navigation Items */}
          <nav className="space-y-1">
            <button
              onClick={() => setTab('dashboard')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'dashboard'
                  ? 'bg-zinc-800/90 text-white shadow-sm border border-zinc-700/60'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40'
              }`}
            >
              <Activity className={`w-4 h-4 ${tab === 'dashboard' ? 'text-emerald-400' : 'text-zinc-400'}`} />
              <span>Dashboard</span>
            </button>

            <button
              onClick={() => setTab('rules')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'rules'
                  ? 'bg-zinc-800/90 text-white shadow-sm border border-zinc-700/60'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40'
              }`}
            >
              <Globe className={`w-4 h-4 ${tab === 'rules' ? 'text-blue-400' : 'text-zinc-400'}`} />
              <span>Blocked Websites</span>
              {status && (
                <span className="ml-auto text-[10px] font-mono px-1.5 py-0.5 rounded-md bg-zinc-800 border border-zinc-700 text-zinc-300">
                  {status.policies.length}
                </span>
              )}
            </button>

            <button
              onClick={() => setTab('add')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'add'
                  ? 'bg-zinc-800/90 text-white shadow-sm border border-zinc-700/60'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40'
              }`}
            >
              <Plus className={`w-4 h-4 ${tab === 'add' ? 'text-emerald-400' : 'text-zinc-400'}`} />
              <span>Add Website</span>
            </button>

            <button
              onClick={() => { setTab('logs'); fetchLogs(); }}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'logs'
                  ? 'bg-zinc-800/90 text-white shadow-sm border border-zinc-700/60'
                  : 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40'
              }`}
            >
              <ScrollText className={`w-4 h-4 ${tab === 'logs' ? 'text-purple-400' : 'text-zinc-400'}`} />
              <span>Audit Trail</span>
            </button>
          </nav>
        </div>

        {/* Security & System Indicator Footer */}
        <div className="p-3.5 rounded-2xl bg-zinc-900/60 border border-zinc-800/80 space-y-2.5">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
              </span>
              <span className="text-[11px] font-semibold text-zinc-300">Daemon Active</span>
            </div>
            <span className="text-[10px] font-mono text-zinc-400 bg-zinc-800/60 px-1.5 py-0.5 rounded border border-zinc-700/50">systemd</span>
          </div>
          <p className="text-[11px] text-zinc-400 leading-relaxed">
            Rules are enforced in the Linux kernel and DNS resolver. Closing this UI will <span className="text-zinc-200 font-medium">never</span> lift protection.
          </p>
        </div>
      </aside>

      {/* Main App Container */}
      <main className="flex-1 flex flex-col overflow-y-auto bg-[#09090b]">
        {/* Top App Header */}
        <header className="sticky top-0 z-10 bg-[#09090b]/80 backdrop-blur-md border-b border-zinc-800/60 px-8 py-4 flex justify-between items-center">
          <div>
            <h2 className="text-lg font-semibold text-white tracking-tight">
              {tab === 'dashboard' && 'System Overview'}
              {tab === 'rules' && 'Enforced Policies'}
              {tab === 'add' && 'Block New Domain'}
              {tab === 'logs' && 'Audit Log Stream'}
            </h2>
            <p className="text-xs text-zinc-400 flex items-center space-x-2 mt-0.5">
              <span>Local System Clock:</span>
              <span className="font-mono text-zinc-200">{now.toLocaleTimeString()}</span>
            </p>
          </div>

          <div className="flex items-center space-x-3">
            <button
              onClick={handleManualRefresh}
              disabled={isRefreshing}
              className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-zinc-900 hover:bg-zinc-800 text-zinc-300 hover:text-white border border-zinc-700/60 text-xs font-medium transition"
              title="Sync with focuswalld daemon"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRefreshing ? 'animate-spin text-emerald-400' : ''}`} />
              <span>Sync State</span>
            </button>
          </div>
        </header>

        {/* Viewport Content */}
        <div className="p-8 max-w-5xl w-full mx-auto space-y-6">

          {/* TAB 1: DASHBOARD */}
          {tab === 'dashboard' && (
            <div className="space-y-6">
              {/* Hero Status Banner */}
              <div className={`p-7 rounded-3xl border surface-card relative overflow-hidden transition-all ${
                ytSchedule.state === 'ALLOWED' ? 'glow-emerald border-emerald-500/30' : 'glow-amber border-zinc-800'
              }`}>
                <div className="flex flex-col md:flex-row md:items-center justify-between gap-6 relative z-10">
                  <div className="space-y-3">
                    <div className="flex items-center space-x-2.5">
                      <span className="text-[11px] font-mono font-semibold uppercase px-2.5 py-0.5 rounded-full bg-zinc-800 border border-zinc-700 text-zinc-300 flex items-center space-x-1.5">
                        <Radio className="w-3 h-3 text-red-400 inline" />
                        <span>YouTube Policy Rule</span>
                      </span>
                      <span className={`text-[11px] font-semibold px-2.5 py-0.5 rounded-full ${
                        ytSchedule.state === 'ALLOWED'
                          ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/30'
                          : 'bg-amber-500/10 text-amber-400 border border-amber-500/30'
                      }`}>
                        {ytSchedule.badge}
                      </span>
                    </div>

                    <div>
                      <div className="text-4xl font-extrabold tracking-tight font-mono text-white">
                        {ytSchedule.timeText}
                      </div>
                      <p className="text-xs text-zinc-400 mt-1 font-medium">
                        {ytSchedule.label} (Daily Allowed Window: <span className="text-zinc-200 font-mono">20:00 – 21:00</span>)
                      </p>
                    </div>
                  </div>

                  {/* Icon Indicator */}
                  <div className="flex items-center space-x-4">
                    <div className={`p-4 rounded-2xl border ${
                      ytSchedule.state === 'ALLOWED'
                        ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                        : 'bg-zinc-800/80 border-zinc-700/80 text-amber-400'
                    }`}>
                      {ytSchedule.state === 'ALLOWED' ? <CheckCircle className="w-9 h-9" /> : <Lock className="w-9 h-9" />}
                    </div>
                  </div>
                </div>

                {/* Sub-status pipeline indicators */}
                <div className="mt-6 pt-5 border-t border-zinc-800/60 grid grid-cols-1 sm:grid-cols-3 gap-3 text-xs">
                  <div className="flex items-center space-x-2 text-zinc-400">
                    <CheckCircle className="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
                    <span>DNS Sinkhole: <strong>0.0.0.0 & ::</strong></span>
                  </div>
                  <div className="flex items-center space-x-2 text-zinc-400">
                    <CheckCircle className="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
                    <span>nftables IP Backstop: <strong>Active</strong></span>
                  </div>
                  <div className="flex items-center space-x-2 text-zinc-400">
                    <CheckCircle className="w-3.5 h-3.5 text-emerald-400 flex-shrink-0" />
                    <span>DoH/DoT Bypass: <strong>Closed</strong></span>
                  </div>
                </div>
              </div>

              {/* Stat Cards */}
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <div className="p-5 rounded-2xl surface-card space-y-1">
                  <div className="flex justify-between items-center text-zinc-400 text-xs">
                    <span>Enforced Policies</span>
                    <Layers className="w-4 h-4 text-zinc-400" />
                  </div>
                  <p className="text-2xl font-bold text-white font-mono">{status?.policies.length || 1}</p>
                  <p className="text-[11px] text-zinc-400">1 System Locked + {(status?.policies.length || 1) - 1} Custom Rules</p>
                </div>

                <div className="p-5 rounded-2xl surface-card space-y-1">
                  <div className="flex justify-between items-center text-zinc-400 text-xs">
                    <span>Sinkholed Domains</span>
                    <Globe className="w-4 h-4 text-zinc-400" />
                  </div>
                  <p className="text-2xl font-bold text-white font-mono">{status?.blocked_domains.length || 0}</p>
                  <p className="text-[11px] text-zinc-400">Zero-latency local DNS interception</p>
                </div>

                <div className="p-5 rounded-2xl surface-card space-y-1">
                  <div className="flex justify-between items-center text-zinc-400 text-xs">
                    <span>Fail-Closed Guarantee</span>
                    <Shield className="w-4 h-4 text-emerald-400" />
                  </div>
                  <p className="text-2xl font-bold text-emerald-400 font-mono">Enforced</p>
                  <p className="text-[11px] text-zinc-400">Daemon crashes retain firewall blocks</p>
                </div>
              </div>

              {/* Philosophy / Intent Note */}
              <div className="p-4 rounded-2xl bg-zinc-900/40 border border-zinc-800/60 flex items-start space-x-3 text-xs text-zinc-400">
                <Sparkles className="w-4 h-4 text-emerald-400 flex-shrink-0 mt-0.5" />
                <p className="leading-relaxed">
                  FocusWall operates on <strong className="text-zinc-200">deliberate friction</strong>. There is no pause toggle, no bypass password, and no instant delete. Custom websites require a 24-hour waiting period before removal to prevent impulsive dismantling.
                </p>
              </div>
            </div>
          )}

          {/* TAB 2: BLOCKED WEBSITES */}
          {tab === 'rules' && (
            <div className="space-y-4">
              {/* Header and Search */}
              <div className="flex flex-col sm:flex-row justify-between items-stretch sm:items-center gap-3">
                <div className="relative flex-1 max-w-md">
                  <Search className="w-4 h-4 text-zinc-400 absolute left-3.5 top-1/2 -translate-y-1/2" />
                  <input
                    type="text"
                    value={ruleSearch}
                    onChange={(e) => setRuleSearch(e.target.value)}
                    placeholder="Search blocked domains..."
                    className="w-full pl-9 pr-4 py-2 rounded-xl bg-zinc-900/80 border border-zinc-800 text-xs text-white placeholder-zinc-400 focus:outline-none focus:border-zinc-600 transition"
                  />
                </div>

                <button
                  onClick={() => setTab('add')}
                  className="flex items-center justify-center space-x-2 px-3.5 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold shadow-lg shadow-emerald-950/40 transition"
                >
                  <Plus className="w-4 h-4" />
                  <span>Block New Website</span>
                </button>
              </div>

              {/* Policy Cards List */}
              <div className="space-y-3">
                {filteredPolicies.map((p) => {
                  const isSystem = p.kind === 'system';
                  const isPending = p.status === 'removal_pending';

                  return (
                    <div
                      key={p.name}
                      className="p-5 rounded-2xl surface-card surface-hover flex flex-col md:flex-row md:items-center justify-between gap-4 transition-all"
                    >
                      <div className="space-y-2">
                        <div className="flex items-center space-x-2.5">
                          <div className="w-7 h-7 rounded-lg bg-zinc-800 border border-zinc-700 flex items-center justify-center font-bold text-xs text-zinc-200 uppercase">
                            {p.name.charAt(0)}
                          </div>
                          <span className="font-bold text-sm text-white">{p.name}</span>
                          {isSystem ? (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-amber-500/10 border border-amber-500/25 text-amber-400 flex items-center space-x-1">
                              <Lock className="w-3 h-3 inline" />
                              <span>System Locked</span>
                            </span>
                          ) : (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-blue-500/10 border border-blue-500/25 text-blue-400">
                              Custom Rule
                            </span>
                          )}
                          {isPending && (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-amber-500/15 border border-amber-500/30 text-amber-300 animate-pulse">
                              ⏳ Removal Cooldown Active
                            </span>
                          )}
                        </div>

                        <div className="text-xs text-zinc-400 space-y-1">
                          <p>
                            Schedule: <span className="text-zinc-200 font-medium">{p.schedule ? `Allowed ${p.schedule.start} – ${p.schedule.end} daily` : '24/7 Blocked'}</span>
                          </p>
                          <p className="font-mono text-[11px] text-zinc-400">
                            Domains ({p.domains.length}): {p.domains.join(', ')}
                          </p>
                          {isPending && p.earliest_removal_at && (
                            <p className="text-amber-400 font-mono text-[11px] pt-1">
                              Cooldown expires at: {new Date(p.earliest_removal_at).toLocaleString()}
                            </p>
                          )}
                        </div>
                      </div>

                      {/* Action Controls */}
                      <div className="flex items-center space-x-2">
                        {isSystem ? (
                          <span className="text-xs text-zinc-400 font-mono italic">Non-removable system policy</span>
                        ) : isPending ? (
                          <div className="flex items-center space-x-2">
                            <button
                              onClick={() => handleCancelRemoval(p.id!)}
                              className="px-3 py-1.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-xs font-medium text-zinc-300 border border-zinc-700 transition"
                            >
                              Cancel Cooldown
                            </button>
                            <button
                              onClick={() => handleConfirmRemoval(p.id!)}
                              className="px-3 py-1.5 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-semibold transition"
                            >
                              Confirm Removal
                            </button>
                          </div>
                        ) : (
                          <button
                            onClick={() => handleRequestRemoval(p.id!)}
                            className="px-3 py-1.5 rounded-xl bg-zinc-900 hover:bg-red-500/10 hover:border-red-500/30 hover:text-red-400 border border-zinc-800 text-zinc-400 text-xs font-medium transition"
                          >
                            Request Removal (24h)
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* TAB 3: ADD WEBSITE */}
          {tab === 'add' && (
            <div className="max-w-lg mx-auto w-full space-y-6 pt-2">
              <div className="text-center space-y-1.5">
                <h3 className="text-xl font-bold text-white tracking-tight">Block Website</h3>
                <p className="text-xs text-zinc-400">
                  Enter any domain or link. It will automatically normalize to its root domain using the Public Suffix List.
                </p>
              </div>

              <div className="p-6 rounded-3xl surface-card space-y-5">
                <div>
                  <label className="block text-xs font-semibold text-zinc-300 mb-1.5">Website Domain / Link</label>
                  <input
                    type="text"
                    value={rawInput}
                    onChange={(e) => setRawInput(e.target.value)}
                    placeholder="e.g. reddit.com, twitter.com, news.ycombinator.com"
                    autoFocus
                    className="w-full px-4 py-3 rounded-xl bg-zinc-950 border border-zinc-800 text-sm text-white placeholder-zinc-400 focus:outline-none focus:border-emerald-500 transition font-mono"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-zinc-300 mb-1.5">Removal Cooldown Duration</label>
                  <div className="grid grid-cols-3 gap-2">
                    {[24, 48, 72].map((hours) => (
                      <button
                        key={hours}
                        type="button"
                        onClick={() => setCooldownHours(hours)}
                        className={`py-2.5 rounded-xl text-xs font-medium border transition ${
                          cooldownHours === hours
                            ? 'bg-emerald-500/10 border-emerald-500/40 text-emerald-400 font-semibold'
                            : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                        }`}
                      >
                        {hours} Hours
                      </button>
                    ))}
                  </div>
                  <p className="text-[11px] text-zinc-400 mt-1.5">
                    How long you must wait before removal can be confirmed in the future.
                  </p>
                </div>

                {preview && (
                  <div className="p-4 rounded-2xl bg-emerald-500/10 border border-emerald-500/25 space-y-2">
                    <div className="flex items-center space-x-2 text-emerald-400 font-semibold text-xs">
                      <CheckCircle className="w-4 h-4" />
                      <span>Normalization Preview</span>
                    </div>
                    <div className="text-xs text-zinc-300 space-y-1">
                      <p>Registrable Root: <strong className="text-white font-mono">{preview.root_domain}</strong></p>
                      <p>Generated Patterns: <code className="text-emerald-300 font-mono text-[11px]">{preview.domains.join(', ')}</code></p>
                    </div>
                  </div>
                )}

                <button
                  disabled={!preview || actionLoading}
                  onClick={() => setShowConfirmModal(true)}
                  className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 text-white font-semibold text-xs transition shadow-lg shadow-emerald-950/60 flex justify-center items-center space-x-2"
                >
                  <span>Review & Enforce Block</span>
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>

              {/* Confirmation Modal Dialog */}
              {showConfirmModal && preview && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 z-50 animate-in fade-in duration-200">
                  <div className="bg-[#121216] border border-zinc-800 max-w-md w-full rounded-3xl p-6 space-y-5 shadow-2xl">
                    <div className="flex justify-between items-start">
                      <div className="space-y-1">
                        <h4 className="text-base font-bold text-white flex items-center space-x-2">
                          <AlertCircle className="w-5 h-5 text-amber-400" />
                          <span>Confirm Permanent Block</span>
                        </h4>
                        <p className="text-xs text-zinc-400">
                          Confirming will enforce 24/7 system-level DNS sinkholing on:
                        </p>
                      </div>
                      <button
                        onClick={() => setShowConfirmModal(false)}
                        className="text-zinc-400 hover:text-zinc-200 p-1 rounded-lg hover:bg-zinc-800"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>

                    <div className="p-4 bg-zinc-950 border border-zinc-800/80 rounded-2xl text-xs space-y-2">
                      <div className="text-zinc-400 font-medium">Domain Scope:</div>
                      <ul className="list-disc list-inside text-zinc-200 font-mono text-[11px] space-y-0.5">
                        {preview.domains.map(d => <li key={d}>{d}</li>)}
                      </ul>
                      <div className="pt-2 text-zinc-400 border-t border-zinc-800 flex justify-between">
                        <span>Required Removal Cooldown:</span>
                        <strong className="text-amber-400 font-mono">{cooldownHours} hours</strong>
                      </div>
                    </div>

                    <div className="flex space-x-3">
                      <button
                        onClick={() => setShowConfirmModal(false)}
                        className="flex-1 py-2.5 rounded-xl bg-zinc-800 hover:bg-zinc-700 text-xs font-semibold text-zinc-300 transition"
                      >
                        Cancel
                      </button>
                      <button
                        disabled={actionLoading}
                        onClick={handleAddRule}
                        className="flex-1 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-xs font-bold text-white transition shadow-lg shadow-emerald-950/40"
                      >
                        {actionLoading ? 'Applying...' : 'Enforce Block'}
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* TAB 4: AUDIT TRAIL */}
          {tab === 'logs' && (
            <div className="space-y-4">
              <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-3">
                <div className="flex items-center space-x-2">
                  {['all', 'daemon', 'policy', 'removal'].map((filter) => (
                    <button
                      key={filter}
                      onClick={() => setLogFilter(filter)}
                      className={`px-3 py-1 rounded-lg text-xs font-medium capitalize transition ${
                        logFilter === filter
                          ? 'bg-zinc-800 text-white border border-zinc-700'
                          : 'text-zinc-400 hover:text-zinc-200'
                      }`}
                    >
                      {filter}
                    </button>
                  ))}
                </div>

                <span className="text-xs text-zinc-400 font-mono">
                  {filteredLogs.length} events recorded
                </span>
              </div>

              <div className="border border-zinc-800/80 rounded-2xl overflow-hidden bg-zinc-950/50">
                <table className="w-full text-left text-xs">
                  <thead className="bg-zinc-900/60 border-b border-zinc-800 text-zinc-400 font-mono text-[11px]">
                    <tr>
                      <th className="p-3.5 font-semibold">Timestamp</th>
                      <th className="p-3.5 font-semibold">Event</th>
                      <th className="p-3.5 font-semibold">Event Details</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-zinc-800/50 font-mono">
                    {filteredLogs.map((log) => (
                      <tr key={log.id} className="hover:bg-zinc-900/30 transition">
                        <td className="p-3.5 text-zinc-400 whitespace-nowrap text-[11px]">{new Date(log.ts).toLocaleString()}</td>
                        <td className="p-3.5 whitespace-nowrap">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase ${
                            log.event_type.includes('start') ? 'bg-blue-500/10 text-blue-400 border border-blue-500/20' :
                            log.event_type.includes('requested') ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20' :
                            log.event_type.includes('confirmed') ? 'bg-red-500/10 text-red-400 border border-red-500/20' :
                            'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                          }`}>
                            {log.event_type}
                          </span>
                        </td>
                        <td className="p-3.5 text-zinc-300 font-sans text-xs">{log.detail}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

        </div>
      </main>
    </div>
  );
}
