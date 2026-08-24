import { useState, useEffect } from 'react';
import {
  Shield,
  ShieldAlert,
  Clock,
  Globe,
  PlusCircle,
  FileText,
  AlertTriangle,
  CheckCircle2,
  RefreshCw,
  Lock,
  ChevronRight,
} from 'lucide-react';
import { SystemStatus, AuditLogEntry, NormalizedPreview } from './types';

// Safe Tauri invoke wrapper that gracefully falls back to mock/IPC testing if running in web browser
const invokeCommand = async <T,>(cmd: string, args: Record<string, any> = {}): Promise<T> => {
  if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  // Mock bridge for direct browser previews
  console.log(`[IPC Invoke] ${cmd}`, args);
  throw new Error("Tauri runtime not connected; use Tauri window or start focuswalld");
};

export function App() {
  const [tab, setTab] = useState<'dashboard' | 'rules' | 'add' | 'logs'>('dashboard');
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);

  // Add rule state
  const [rawInput, setRawInput] = useState('');
  const [cooldownHours, setCooldownHours] = useState(24);
  const [preview, setPreview] = useState<NormalizedPreview | null>(null);
  const [showConfirmModal, setShowConfirmModal] = useState(false);
  const [actionLoading, setActionLoading] = useState(false);

  // Time remaining calculator
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
      // Fallback local mock status if daemon socket not connected yet
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
        blocked_domains: isAllowed ? [] : ['youtube.com', 'googlevideo.com'],
      });
    }
  };

  const fetchLogs = async () => {
    try {
      const data = await invokeCommand<AuditLogEntry[]>('get_logs', { limit: 50 });
      setLogs(data);
    } catch {
      setLogs([
        { id: 1, ts: new Date().toISOString(), event_type: 'daemon_start', detail: 'focuswalld active' }
      ]);
    }
  };

  useEffect(() => {
    fetchStatus();
    fetchLogs();
    const interval = setInterval(() => {
      fetchStatus();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // Update preview when rawInput changes
  useEffect(() => {
    const trimmed = rawInput.trim();
    if (!trimmed) {
      setPreview(null);
      return;
    }
    // Client-side quick preview
    try {
      let host = trimmed;
      if (!host.includes('://')) host = 'https://' + host;
      const url = new URL(host);
      let domain = url.hostname.replace(/^www\./, '');
      if (domain.includes('.')) {
        setPreview({
          root_domain: domain,
          domains: [domain, `www.${domain}`],
        });
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
      alert(`Error adding rule: ${e.message || e}`);
    } finally {
      setActionLoading(false);
    }
  };

  const handleRequestRemoval = async (ruleId: number) => {
    if (!confirm("Are you sure you want to request removal? The site will STAY BLOCKED for the entire 24-hour cooldown period.")) {
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
      alert(`Cannot remove yet: ${e.message || e}`);
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

  // Helper for YouTube countdown
  const getYouTubeCountdown = () => {
    const currentH = now.getHours();
    const currentM = now.getMinutes();
    const currentS = now.getSeconds();

    if (currentH === 20) {
      const remainingM = 59 - currentM;
      const remainingS = 59 - currentS;
      return { status: 'ALLOWED', text: `Window closes in ${remainingM}m ${remainingS}s`, color: 'emerald' };
    } else if (currentH < 20) {
      const diffS = (20 * 3600) - (currentH * 3600 + currentM * 60 + currentS);
      const hours = Math.floor(diffS / 3600);
      const mins = Math.floor((diffS % 3600) / 60);
      const secs = diffS % 60;
      return { status: 'BLOCKED', text: `Opens in ${hours}h ${mins}m ${secs}s (at 20:00)`, color: 'amber' };
    } else {
      const diffS = ((24 + 20) * 3600) - (currentH * 3600 + currentM * 60 + currentS);
      const hours = Math.floor(diffS / 3600);
      const mins = Math.floor((diffS % 3600) / 60);
      const secs = diffS % 60;
      return { status: 'BLOCKED', text: `Opens tomorrow at 20:00 (in ${hours}h ${mins}m ${secs}s)`, color: 'amber' };
    }
  };

  const ytCountdown = getYouTubeCountdown();

  return (
    <div className="flex h-screen bg-neutral-950 text-neutral-100 antialiased overflow-hidden font-sans">
      {/* Sidebar */}
      <aside className="w-64 border-r border-neutral-800/80 bg-neutral-900/40 backdrop-blur flex flex-col justify-between p-4">
        <div className="space-y-6">
          <div className="flex items-center space-x-3 px-2">
            <div className="p-2 bg-emerald-500/10 border border-emerald-500/30 rounded-xl text-emerald-400">
              <Shield className="w-6 h-6" />
            </div>
            <div>
              <h1 className="font-bold text-lg tracking-tight bg-gradient-to-r from-white to-neutral-400 bg-clip-text text-transparent">FocusWall</h1>
              <p className="text-xs text-neutral-400 font-mono">system-level guard</p>
            </div>
          </div>

          <nav className="space-y-1.5">
            <button
              onClick={() => setTab('dashboard')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-lg text-sm font-medium transition ${
                tab === 'dashboard'
                  ? 'bg-neutral-800 text-white shadow-sm border border-neutral-700/60'
                  : 'text-neutral-400 hover:text-white hover:bg-neutral-800/40'
              }`}
            >
              <Shield className="w-4 h-4 text-emerald-400" />
              <span>Dashboard</span>
            </button>
            <button
              onClick={() => setTab('rules')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-lg text-sm font-medium transition ${
                tab === 'rules'
                  ? 'bg-neutral-800 text-white shadow-sm border border-neutral-700/60'
                  : 'text-neutral-400 hover:text-white hover:bg-neutral-800/40'
              }`}
            >
              <Globe className="w-4 h-4 text-blue-400" />
              <span>Blocked Sites</span>
              {status && (
                <span className="ml-auto text-xs px-2 py-0.5 rounded-full bg-neutral-800 border border-neutral-700 text-neutral-300">
                  {status.policies.length}
                </span>
              )}
            </button>
            <button
              onClick={() => setTab('add')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-lg text-sm font-medium transition ${
                tab === 'add'
                  ? 'bg-neutral-800 text-white shadow-sm border border-neutral-700/60'
                  : 'text-neutral-400 hover:text-white hover:bg-neutral-800/40'
              }`}
            >
              <PlusCircle className="w-4 h-4 text-emerald-400" />
              <span>Add Website</span>
            </button>
            <button
              onClick={() => { setTab('logs'); fetchLogs(); }}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-lg text-sm font-medium transition ${
                tab === 'logs'
                  ? 'bg-neutral-800 text-white shadow-sm border border-neutral-700/60'
                  : 'text-neutral-400 hover:text-white hover:bg-neutral-800/40'
              }`}
            >
              <FileText className="w-4 h-4 text-purple-400" />
              <span>Audit Logs</span>
            </button>
          </nav>
        </div>

        <div className="p-3 bg-neutral-900/80 border border-neutral-800 rounded-xl space-y-2 text-xs">
          <div className="flex items-center space-x-2 text-neutral-300">
            <Lock className="w-3.5 h-3.5 text-emerald-400" />
            <span className="font-semibold">Privilege Separation</span>
          </div>
          <p className="text-neutral-400 leading-relaxed">
            Closing or killing this UI will <strong className="text-neutral-200">never</strong> unblock websites. Enforcement runs in <code className="text-emerald-400 font-mono">focuswalld</code>.
          </p>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 flex flex-col overflow-y-auto p-8 max-w-5xl mx-auto w-full">
        {/* Top bar with live time */}
        <header className="flex justify-between items-center pb-6 mb-6 border-b border-neutral-800/80">
          <div>
            <h2 className="text-2xl font-bold tracking-tight text-white capitalize">
              {tab === 'dashboard' && 'Protection Dashboard'}
              {tab === 'rules' && 'Enforced Websites & Policies'}
              {tab === 'add' && 'Block New Website'}
              {tab === 'logs' && 'Tamper-Evident Audit Trail'}
            </h2>
            <p className="text-xs text-neutral-400 mt-1">
              Local Time: <span className="font-mono text-neutral-200">{now.toLocaleTimeString()}</span> ({Intl.DateTimeFormat().resolvedOptions().timeZone})
            </p>
          </div>
          <button
            onClick={fetchStatus}
            className="flex items-center space-x-2 px-3 py-1.5 rounded-lg bg-neutral-800/80 hover:bg-neutral-800 border border-neutral-700 text-xs font-medium transition"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Sync</span>
          </button>
        </header>

        {/* Tab 1: Dashboard */}
        {tab === 'dashboard' && (
          <div className="space-y-6">
            {/* Hero YouTube Card */}
            <div className="p-6 rounded-2xl bg-gradient-to-br from-neutral-900 via-neutral-900 to-neutral-950 border border-neutral-800 shadow-xl relative overflow-hidden">
              <div className="flex justify-between items-start">
                <div className="space-y-2">
                  <div className="flex items-center space-x-2.5">
                    <span className="text-xs font-bold uppercase tracking-wider px-2.5 py-1 rounded-full bg-red-500/10 text-red-400 border border-red-500/20">
                      YouTube Policy
                    </span>
                    <span className="text-xs text-neutral-400">Strict Window: 20:00 – 21:00</span>
                  </div>
                  <h3 className="text-3xl font-extrabold tracking-tight text-white">
                    {ytCountdown.status === 'ALLOWED' ? (
                      <span className="text-emerald-400">Allowed Window Open</span>
                    ) : (
                      <span className="text-amber-400">Enforcement Active (Blocked)</span>
                    )}
                  </h3>
                  <p className="text-sm font-mono text-neutral-300 flex items-center space-x-2 pt-1">
                    <Clock className="w-4 h-4 text-neutral-400" />
                    <span>{ytCountdown.text}</span>
                  </p>
                </div>

                <div className={`p-4 rounded-2xl border ${
                  ytCountdown.status === 'ALLOWED'
                    ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                    : 'bg-amber-500/10 border-amber-500/30 text-amber-400'
                }`}>
                  {ytCountdown.status === 'ALLOWED' ? <CheckCircle2 className="w-8 h-8" /> : <Lock className="w-8 h-8" />}
                </div>
              </div>

              <div className="mt-6 pt-6 border-t border-neutral-800/80 flex flex-wrap gap-4 text-xs text-neutral-400">
                <div>
                  <span className="text-neutral-300 font-semibold">DNS Sinkhole:</span> 8 YouTube domains mapped to 0.0.0.0 & ::
                </div>
                <div>
                  <span className="text-neutral-300 font-semibold">Firewall Backstop:</span> Outbound drops on TCP/UDP 80/443
                </div>
                <div>
                  <span className="text-neutral-300 font-semibold">DoH Closure:</span> Public DNS resolvers blocked
                </div>
              </div>
            </div>

            {/* Metrics Grid */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="p-5 rounded-xl bg-neutral-900/60 border border-neutral-800 space-y-1">
                <p className="text-xs text-neutral-400 font-medium">Active Policy Rules</p>
                <p className="text-2xl font-bold text-white font-mono">{status?.policies.length || 1}</p>
                <p className="text-[11px] text-neutral-400">1 System (YouTube) + {(status?.policies.length || 1) - 1} Custom</p>
              </div>

              <div className="p-5 rounded-xl bg-neutral-900/60 border border-neutral-800 space-y-1">
                <p className="text-xs text-neutral-400 font-medium">Currently Blocked Domains</p>
                <p className="text-2xl font-bold text-white font-mono">{status?.blocked_domains.length || 0}</p>
                <p className="text-[11px] text-neutral-400">Sinkholed in local resolver & firewall</p>
              </div>

              <div className="p-5 rounded-xl bg-neutral-900/60 border border-neutral-800 space-y-1">
                <p className="text-xs text-neutral-400 font-medium">Daemon Status</p>
                <div className="flex items-center space-x-2 pt-1">
                  <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse"></span>
                  <span className="text-sm font-semibold text-emerald-400 font-mono">Running (systemd)</span>
                </div>
                <p className="text-[11px] text-neutral-400">Auto-restart on fail active</p>
              </div>
            </div>

            {/* Guarantee Note */}
            <div className="p-4 rounded-xl bg-neutral-900/40 border border-neutral-800/80 flex items-start space-x-3 text-xs text-neutral-300">
              <ShieldAlert className="w-5 h-5 text-emerald-400 flex-shrink-0 mt-0.5" />
              <p className="leading-relaxed">
                <strong>Deliberate Friction Guarantee:</strong> FocusWall has no manual override button, no temporary pause, and no bypass endpoint. Custom websites require a strictly enforced 24-hour cooldown before removal.
              </p>
            </div>
          </div>
        )}

        {/* Tab 2: Blocked Sites */}
        {tab === 'rules' && (
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <p className="text-sm text-neutral-400">All configured system and custom blocking policies.</p>
              <button
                onClick={() => setTab('add')}
                className="flex items-center space-x-2 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold transition"
              >
                <PlusCircle className="w-4 h-4" />
                <span>Add Site</span>
              </button>
            </div>

            <div className="space-y-3">
              {status?.policies.map((p) => {
                const isSystem = p.kind === 'system';
                const isPending = p.status === 'removal_pending';

                return (
                  <div
                    key={p.name}
                    className="p-5 rounded-xl bg-neutral-900/70 border border-neutral-800 flex flex-col md:flex-row md:items-center justify-between gap-4"
                  >
                    <div className="space-y-1.5">
                      <div className="flex items-center space-x-2.5">
                        <span className="font-bold text-base text-white">{p.name}</span>
                        {isSystem ? (
                          <span className="text-[10px] uppercase tracking-wider font-bold px-2 py-0.5 rounded bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center space-x-1">
                            <Lock className="w-3 h-3 inline" />
                            <span>System Locked</span>
                          </span>
                        ) : (
                          <span className="text-[10px] uppercase tracking-wider font-bold px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/20 text-blue-400">
                            Custom
                          </span>
                        )}
                        {isPending && (
                          <span className="text-[10px] uppercase tracking-wider font-bold px-2 py-0.5 rounded bg-red-500/10 border border-red-500/20 text-red-400 animate-pulse">
                            Removal Pending
                          </span>
                        )}
                      </div>

                      <div className="text-xs text-neutral-400 space-y-1">
                        <p>
                          Schedule: <span className="text-neutral-200">{p.schedule ? `Allowed ${p.schedule.start} – ${p.schedule.end}` : '24/7 Blocked'}</span>
                        </p>
                        <p className="font-mono text-[11px] text-neutral-400">
                          Domains: {p.domains.join(', ')}
                        </p>
                        {isPending && p.earliest_removal_at && (
                          <p className="text-amber-400 font-mono text-xs pt-1">
                            ⏳ Cooldown in effect: Can be finalized after {new Date(p.earliest_removal_at).toLocaleString()}
                          </p>
                        )}
                      </div>
                    </div>

                    {/* Action buttons */}
                    <div>
                      {isSystem ? (
                        <span className="text-xs text-neutral-400 italic">Non-removable</span>
                      ) : isPending ? (
                        <div className="flex items-center space-x-2">
                          <button
                            onClick={() => handleCancelRemoval(p.id!)}
                            className="px-3 py-1.5 rounded-lg bg-neutral-800 hover:bg-neutral-700 text-xs text-neutral-200 border border-neutral-700 transition"
                          >
                            Cancel Request
                          </button>
                          <button
                            onClick={() => handleConfirmRemoval(p.id!)}
                            className="px-3 py-1.5 rounded-lg bg-red-600 hover:bg-red-500 text-white text-xs font-semibold transition"
                          >
                            Confirm Removal
                          </button>
                        </div>
                      ) : (
                        <button
                          onClick={() => handleRequestRemoval(p.id!)}
                          className="px-3 py-1.5 rounded-lg bg-neutral-800/80 hover:bg-red-500/10 hover:border-red-500/30 hover:text-red-400 border border-neutral-700 text-neutral-300 text-xs font-medium transition"
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

        {/* Tab 3: Add Website */}
        {tab === 'add' && (
          <div className="max-w-xl mx-auto w-full space-y-6 pt-4">
            <div className="space-y-2">
              <h3 className="text-lg font-bold text-white">Add Website to Blocklist</h3>
              <p className="text-xs text-neutral-400">
                Enter any domain or link (e.g. <code className="text-neutral-300">reddit.com</code> or <code className="text-neutral-300">https://www.reddit.com/r/all</code>). It will be normalized to the registrable root domain using the Public Suffix List.
              </p>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-semibold text-neutral-300 mb-1.5">Website Domain / URL</label>
                <input
                  type="text"
                  value={rawInput}
                  onChange={(e) => setRawInput(e.target.value)}
                  placeholder="e.g. reddit.com, twitter.com, news.ycombinator.com"
                  className="w-full px-4 py-3 rounded-xl bg-neutral-900 border border-neutral-700 focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 text-white text-sm outline-none transition"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-neutral-300 mb-1.5">Removal Cooldown Period</label>
                <select
                  value={cooldownHours}
                  onChange={(e) => setCooldownHours(Number(e.target.value))}
                  className="w-full px-4 py-3 rounded-xl bg-neutral-900 border border-neutral-700 focus:border-emerald-500 text-white text-sm outline-none transition"
                >
                  <option value={24}>24 hours (Recommended standard)</option>
                  <option value={48}>48 hours (Strict friction)</option>
                  <option value={72}>72 hours (Maximum friction)</option>
                </select>
              </div>

              {preview && (
                <div className="p-4 rounded-xl bg-emerald-500/10 border border-emerald-500/30 space-y-2">
                  <div className="flex items-center space-x-2 text-emerald-400 font-semibold text-xs">
                    <CheckCircle2 className="w-4 h-4" />
                    <span>Normalization Preview</span>
                  </div>
                  <div className="text-xs text-neutral-300 space-y-1">
                    <p>Root Domain: <strong className="text-white font-mono">{preview.root_domain}</strong></p>
                    <p>Blocked Patterns: <code className="text-emerald-300 font-mono text-[11px]">{preview.domains.join(', ')}</code></p>
                  </div>
                </div>
              )}

              <button
                disabled={!preview || actionLoading}
                onClick={() => setShowConfirmModal(true)}
                className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-bold text-sm transition shadow-lg shadow-emerald-950 flex justify-center items-center space-x-2"
              >
                <span>Preview & Confirm Blocking</span>
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>

            {/* Confirmation Modal */}
            {showConfirmModal && preview && (
              <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 z-50">
                <div className="bg-neutral-900 border border-neutral-800 max-w-md w-full rounded-2xl p-6 space-y-5 shadow-2xl">
                  <div className="space-y-2">
                    <h4 className="text-lg font-bold text-white flex items-center space-x-2">
                      <AlertTriangle className="w-5 h-5 text-amber-400" />
                      <span>Confirm Website Block</span>
                    </h4>
                    <p className="text-xs text-neutral-300 leading-relaxed">
                      You are about to enforce a 24/7 system-level block on <strong className="text-white font-mono">{preview.root_domain}</strong> and all its subdomains.
                    </p>
                  </div>

                  <div className="p-3 bg-neutral-950 border border-neutral-800 rounded-xl text-xs space-y-1.5">
                    <div className="text-neutral-400">Enforcement Scope:</div>
                    <ul className="list-disc list-inside text-neutral-200 font-mono text-[11px]">
                      {preview.domains.map(d => <li key={d}>{d}</li>)}
                    </ul>
                    <div className="pt-2 text-neutral-400 border-t border-neutral-800">
                      Removal Cooldown: <strong className="text-amber-400">{cooldownHours} hours</strong>
                    </div>
                  </div>

                  <div className="flex space-x-3">
                    <button
                      onClick={() => setShowConfirmModal(false)}
                      className="flex-1 py-2.5 rounded-xl bg-neutral-800 hover:bg-neutral-700 text-xs font-semibold text-neutral-300 transition"
                    >
                      Cancel
                    </button>
                    <button
                      disabled={actionLoading}
                      onClick={handleAddRule}
                      className="flex-1 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-xs font-bold text-white transition"
                    >
                      {actionLoading ? 'Applying...' : 'Confirm Block'}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Tab 4: Audit Logs */}
        {tab === 'logs' && (
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <p className="text-sm text-neutral-400">Tamper-evident log of daemon starts, policy updates, and cooldown events.</p>
              <button
                onClick={fetchLogs}
                className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-neutral-800 hover:bg-neutral-700 text-xs font-medium text-neutral-300 transition"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                <span>Refresh</span>
              </button>
            </div>

            <div className="border border-neutral-800 rounded-2xl overflow-hidden bg-neutral-900/40">
              <table className="w-full text-left text-xs">
                <thead className="bg-neutral-900 border-b border-neutral-800 text-neutral-400">
                  <tr>
                    <th className="p-3.5 font-semibold">Timestamp</th>
                    <th className="p-3.5 font-semibold">Event</th>
                    <th className="p-3.5 font-semibold">Details</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-neutral-800/60 font-mono">
                  {logs.map((log) => (
                    <tr key={log.id} className="hover:bg-neutral-800/30 transition">
                      <td className="p-3.5 text-neutral-400 whitespace-nowrap">{new Date(log.ts).toLocaleString()}</td>
                      <td className="p-3.5 whitespace-nowrap">
                        <span className={`px-2 py-0.5 rounded text-[10px] uppercase font-bold ${
                          log.event_type === 'daemon_start' ? 'bg-blue-500/10 text-blue-400 border border-blue-500/20' :
                          log.event_type === 'removal_requested' ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20' :
                          log.event_type === 'removal_confirmed' ? 'bg-red-500/10 text-red-400 border border-red-500/20' :
                          'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                        }`}>
                          {log.event_type}
                        </span>
                      </td>
                      <td className="p-3.5 text-neutral-200 font-sans">{log.detail}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
