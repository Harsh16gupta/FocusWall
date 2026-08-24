import { useState, useEffect, useMemo } from 'react';
import {
  Shield,
  ShieldCheck,
  Globe,
  Plus,
  ScrollText,
  AlertCircle,
  CheckCircle,
  RefreshCw,
  Lock,
  Search,
  ChevronRight,
  Activity,
  X,
  Radio,
  FileText,
  Clock,
  Info,
  Settings as SettingsIcon,
  Sun,
  Moon,
  ArrowRight,
  Check,
  Zap,
} from 'lucide-react';
import { SystemStatus, AuditLogEntry, NormalizedPreview } from './types';

// Safe Tauri invoke wrapper that falls back to mock/local state in browser preview
const invokeCommand = async <T,>(cmd: string, args: Record<string, any> = {}): Promise<T> => {
  if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  console.log(`[IPC Invoke] ${cmd}`, args);
  throw new Error("Tauri runtime not connected; using local reactive state");
};

// Hourly data for the 24h blocked requests chart matching the visual in the reference image
const hourlyBlockedData = [
  { hour: '00:00', count: 12 },
  { hour: '01:00', count: 28 },
  { hour: '02:00', count: 48 },
  { hour: '03:00', count: 15 },
  { hour: '04:00', count: 72 },
  { hour: '05:00', count: 40 },
  { hour: '06:00', count: 65 },
  { hour: '07:00', count: 98 },
  { hour: '08:00', count: 92 },
  { hour: '09:00', count: 52 },
  { hour: '10:00', count: 32 },
  { hour: '11:00', count: 20 },
  { hour: '12:00', count: 68 },
  { hour: '13:00', count: 104 },
  { hour: '14:00', count: 75 },
  { hour: '15:00', count: 42 },
  { hour: '16:00', count: 62 },
  { hour: '17:00', count: 88 },
  { hour: '18:00', count: 58 },
  { hour: '19:00', count: 76 },
  { hour: '20:00', count: 38 },
  { hour: '21:00', count: 18 },
  { hour: '22:00', count: 24 },
  { hour: '23:00', count: 10 },
];

export function App() {
  const [tab, setTab] = useState<'dashboard' | 'rules' | 'add' | 'logs' | 'settings'>('dashboard');
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  const [logFilter, setLogFilter] = useState<string>('all');
  const [ruleSearch, setRuleSearch] = useState<string>('');
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isDarkMode, setIsDarkMode] = useState<boolean>(() => {
    if (typeof window !== 'undefined') {
      const saved = localStorage.getItem('focuswall_theme');
      if (saved) return saved === 'dark';
      return false; // Default to Light mode to match reference image directly
    }
    return false;
  });

  // Modal dialog states
  const [activeModal, setActiveModal] = useState<'domains' | 'fail_closed' | 'top_domains' | null>(null);

  // Add rule form state
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

  // Synchronize dark mode class with HTML document root
  useEffect(() => {
    const root = document.documentElement;
    if (isDarkMode) {
      root.classList.add('dark');
      localStorage.setItem('focuswall_theme', 'dark');
    } else {
      root.classList.remove('dark');
      localStorage.setItem('focuswall_theme', 'light');
    }
  }, [isDarkMode]);

  const toggleTheme = () => {
    setIsDarkMode((prev) => !prev);
  };

  const fetchStatus = async () => {
    try {
      const data = await invokeCommand<SystemStatus>('get_status');
      setStatus(data);
    } catch {
      // Fallback local status for browser preview / mock mode
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
        blocked_domains: isAllowed ? [] : ['youtube.com', 'www.youtube.com', 'i.ytimg.com', 'm.youtube.com'],
      });
    }
  };

  const fetchLogs = async () => {
    try {
      const data = await invokeCommand<AuditLogEntry[]>('get_logs', { limit: 50 });
      setLogs(data);
    } catch {
      setLogs([
        { id: 1, ts: new Date(Date.now() - 3600000 * 2).toISOString(), event_type: 'daemon_start', detail: 'focuswalld active with fail-closed kernel protection' },
        { id: 2, ts: new Date(Date.now() - 3600000 * 1.5).toISOString(), event_type: 'dns_intercept', detail: 'Sinkholed query for youtube.com -> 0.0.0.0' },
        { id: 3, ts: new Date(Date.now() - 3600000).toISOString(), event_type: 'policy_enforce', detail: 'Daily locked schedule active: YouTube blocked outside 20:00-21:00' },
        { id: 4, ts: new Date(Date.now() - 1800000).toISOString(), event_type: 'dns_intercept', detail: 'Sinkholed query for i.ytimg.com -> 0.0.0.0' },
      ]);
    }
  };

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    await Promise.all([fetchStatus(), fetchLogs()]);
    setTimeout(() => setIsRefreshing(false), 450);
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
      const hostname = url.hostname.toLowerCase();
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
      // In standalone web preview, simulate rule addition
      if (status) {
        const newPolicy = {
          id: Date.now(),
          kind: 'custom' as const,
          name: preview.root_domain,
          domains: preview.domains,
          timezone: 'system',
          status: 'active' as const,
          created_at: new Date().toISOString(),
          removal_cooldown_hours: cooldownHours,
        };
        setStatus({
          ...status,
          policies: [...status.policies, newPolicy],
          blocked_domains: [...status.blocked_domains, ...preview.domains],
        });
      }
      setRawInput('');
      setPreview(null);
      setShowConfirmModal(false);
      setTab('rules');
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
      if (status) {
        const earliest = new Date(Date.now() + 24 * 3600 * 1000).toISOString();
        setStatus({
          ...status,
          policies: status.policies.map(p => p.id === ruleId ? {
            ...p,
            status: 'removal_pending',
            removal_requested_at: new Date().toISOString(),
            earliest_removal_at: earliest,
          } : p),
        });
      }
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
      if (status) {
        setStatus({
          ...status,
          policies: status.policies.map(p => p.id === ruleId ? {
            ...p,
            status: 'active',
            removal_requested_at: undefined,
            earliest_removal_at: undefined,
          } : p),
        });
      }
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
      return {
        state: 'ALLOWED',
        badge: 'Window Open',
        timeText: `${String(remainingM).padStart(2, '0')}:${String(remainingS).padStart(2, '0')}`,
        label: 'remaining before lock',
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
      return {
        state: 'BLOCKED',
        badge: 'Strictly Enforced',
        timeText: `${hours}h ${String(mins).padStart(2, '0')}m ${String(secs).padStart(2, '0')}s`,
        label: 'until 20:00 unlock window',
      };
    }
  }, [now]);

  const customPolicyCount = (status?.policies?.length || 1) - 1;
  const sinkholedDomainCount = status?.blocked_domains?.length || 4;

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

  // Format system clock string matching screenshot format: 8:25:42 AM
  const formattedSystemClock = useMemo(() => {
    return now.toLocaleTimeString('en-US', {
      hour: 'numeric',
      minute: '2-digit',
      second: '2-digit',
      hour12: true,
    });
  }, [now]);

  return (
    <div className="flex h-screen bg-[#f8fafc] dark:bg-[#09090b] text-zinc-900 dark:text-[#f4f4f5] antialiased overflow-hidden font-sans select-none transition-colors duration-200">
      {/* Sidebar Navigation */}
      <aside className="w-64 border-r border-slate-200/90 dark:border-zinc-800/80 bg-white dark:bg-[#0d0d11] flex flex-col justify-between p-4 z-20 transition-colors duration-200">
        <div className="space-y-6">
          {/* App Branding */}
          <div className="flex items-center space-x-3 px-2 pt-1">
            <div className="w-9 h-9 rounded-xl bg-emerald-50 dark:bg-emerald-500/10 border border-emerald-200/80 dark:border-emerald-500/20 flex items-center justify-center text-emerald-600 dark:text-emerald-400 shadow-sm">
              <Shield className="w-5 h-5 fill-emerald-600/15 dark:fill-emerald-400/20" />
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <span className="font-bold text-sm tracking-tight text-zinc-900 dark:text-white">FocusWall</span>
                <span className="text-[10px] font-mono px-1.5 py-0.5 bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400 rounded border border-zinc-200 dark:border-zinc-700 leading-none">
                  v1.0.0
                </span>
              </div>
              <p className="text-xs text-zinc-400 dark:text-zinc-500">System-Level Guard</p>
            </div>
          </div>

          {/* Navigation Items */}
          <nav className="space-y-1">
            <button
              onClick={() => setTab('dashboard')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'dashboard'
                  ? 'bg-emerald-50/90 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 font-semibold'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50'
              }`}
            >
              <Activity className={`w-4 h-4 ${tab === 'dashboard' ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'}`} />
              <span>Dashboard</span>
            </button>

            <button
              onClick={() => setTab('rules')}
              className={`w-full flex items-center justify-between px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'rules'
                  ? 'bg-emerald-50/90 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 font-semibold'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50'
              }`}
            >
              <div className="flex items-center space-x-3">
                <Globe className={`w-4 h-4 ${tab === 'rules' ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'}`} />
                <span>Blocked Websites</span>
              </div>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-zinc-100 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 text-zinc-600 dark:text-zinc-300">
                {status?.policies?.length || 1}
              </span>
            </button>

            <button
              onClick={() => setTab('add')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'add'
                  ? 'bg-emerald-50/90 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 font-semibold'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50'
              }`}
            >
              <Plus className={`w-4 h-4 ${tab === 'add' ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'}`} />
              <span>Add Website</span>
            </button>

            <button
              onClick={() => { setTab('logs'); fetchLogs(); }}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'logs'
                  ? 'bg-emerald-50/90 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 font-semibold'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50'
              }`}
            >
              <FileText className={`w-4 h-4 ${tab === 'logs' ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'}`} />
              <span>Audit Trail</span>
            </button>

            <button
              onClick={() => setTab('settings')}
              className={`w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-medium transition-all ${
                tab === 'settings'
                  ? 'bg-emerald-50/90 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 font-semibold'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50'
              }`}
            >
              <SettingsIcon className={`w-4 h-4 ${tab === 'settings' ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'}`} />
              <span>Settings</span>
            </button>
          </nav>
        </div>

        {/* Security & System Indicator Box */}
        <div className="space-y-4">
          <div className="p-3.5 rounded-2xl bg-zinc-50/80 dark:bg-zinc-900/60 border border-zinc-200/90 dark:border-zinc-800/80 space-y-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="relative flex h-2 w-2">
                  <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                  <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
                </span>
                <span className="text-xs font-semibold text-zinc-800 dark:text-zinc-200">Daemon Active</span>
              </div>
              <span className="text-[10px] font-mono text-zinc-500 dark:text-zinc-400 bg-zinc-100 dark:bg-zinc-800/80 px-1.5 py-0.5 rounded border border-zinc-200 dark:border-zinc-700/60">
                systemd
              </span>
            </div>
            <p className="text-xs text-zinc-500 dark:text-zinc-400 leading-relaxed">
              FocusWall runs at the kernel level using nftables and DNS filtering. Closing this UI will never lift protection.
            </p>
          </div>

          <div className="px-1 text-xs text-zinc-400 dark:text-zinc-500">
            © 2025 FocusWall
          </div>
        </div>
      </aside>

      {/* Main Container */}
      <main className="flex-1 flex flex-col overflow-y-auto bg-[#f8fafc] dark:bg-[#09090b] transition-colors duration-200">
        {/* Top App Header */}
        <header className="sticky top-0 z-10 bg-[#f8fafc]/90 dark:bg-[#09090b]/90 backdrop-blur-md px-8 py-5 flex justify-between items-center transition-colors duration-200">
          <div>
            <div className="flex items-center space-x-2.5">
              <h1 className="text-xl font-bold text-zinc-900 dark:text-white tracking-tight">
                {tab === 'dashboard' && 'System Overview'}
                {tab === 'rules' && 'Enforced Policies'}
                {tab === 'add' && 'Add Blocked Website'}
                {tab === 'logs' && 'Audit Log Stream'}
                {tab === 'settings' && 'System Settings'}
              </h1>
              <span className="inline-flex items-center space-x-1.5 text-xs font-semibold text-emerald-600 dark:text-emerald-400">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
                <span>All systems secure</span>
              </span>
            </div>
            <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1">
              Local System Clock: <span className="font-medium text-zinc-700 dark:text-zinc-300">{formattedSystemClock}</span>
            </p>
          </div>

          <div className="flex items-center space-x-3">
            {/* Theme Toggle Button */}
            <button
              onClick={toggleTheme}
              className="p-2 rounded-xl bg-white dark:bg-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 text-zinc-600 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700/80 shadow-sm transition"
              title={isDarkMode ? "Switch to Light Mode" : "Switch to Dark Mode"}
            >
              {isDarkMode ? <Sun className="w-4 h-4 text-amber-400" /> : <Moon className="w-4 h-4 text-zinc-600" />}
            </button>

            {/* Sync State Button */}
            <button
              onClick={handleManualRefresh}
              disabled={isRefreshing}
              className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-xl bg-white dark:bg-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700/80 text-xs font-medium shadow-sm transition"
              title="Sync with focuswalld daemon"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRefreshing ? 'animate-spin text-emerald-600' : 'text-zinc-500 dark:text-zinc-400'}`} />
              <span>Sync State</span>
            </button>
          </div>
        </header>

        {/* Viewport Content */}
        <div className="px-8 pb-8 max-w-6xl w-full mx-auto space-y-6 flex-1">

          {/* TAB 1: DASHBOARD (EXACT REFERENCE DESIGN) */}
          {tab === 'dashboard' && (
            <div className="space-y-5 animate-in fade-in duration-150">
              {/* Hero Status Container */}
              <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-3xl p-7 shadow-[0_1px_3px_rgba(0,0,0,0.02)] space-y-6">
                <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
                  {/* Left Column: YouTube Policy Badge & Huge Countdown */}
                  <div className="space-y-2.5">
                    <div className="flex items-center space-x-2.5">
                      <span className="text-xs font-bold uppercase tracking-wide text-red-500 flex items-center space-x-1.5">
                        <Radio className="w-3.5 h-3.5 text-red-500" />
                        <span>YOUTUBE POLICY RULE</span>
                      </span>
                      <span className="text-xs font-medium px-2.5 py-0.5 rounded-full bg-amber-50 dark:bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-200/80 dark:border-amber-500/25">
                        {ytSchedule.badge}
                      </span>
                    </div>

                    <div>
                      <div className="text-[40px] leading-tight font-bold tracking-tight text-zinc-900 dark:text-white">
                        {ytSchedule.timeText}
                      </div>
                      <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1">
                        {ytSchedule.label}
                      </p>
                      <p className="text-xs text-zinc-600 dark:text-zinc-300 mt-0.5">
                        Daily Allowed Window: <span className="text-emerald-600 dark:text-emerald-400 font-medium">20:00 – 21:00</span>
                      </p>
                    </div>
                  </div>

                  {/* Right Column: Circular Lock Icon & Protection Status */}
                  <div className="flex items-center space-x-4">
                    <div className="w-20 h-20 rounded-full border border-zinc-200/90 dark:border-zinc-700/80 bg-zinc-50/80 dark:bg-zinc-800/40 flex items-center justify-center flex-shrink-0">
                      <Lock className="w-7 h-7 text-zinc-900 dark:text-white stroke-[2.2]" />
                    </div>
                    <div>
                      <p className="text-xs text-zinc-400 dark:text-zinc-500">Protection Status</p>
                      <h3 className="text-xl font-bold text-zinc-900 dark:text-white tracking-wide uppercase">
                        {ytSchedule.state === 'ALLOWED' ? 'UNLOCKED' : 'LOCKED'}
                      </h3>
                      <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1 max-w-[200px] leading-snug">
                        All restricted access is blocked outside the allowed window.
                      </p>
                    </div>
                  </div>
                </div>

                {/* Sub-status Row with subtle dividers */}
                <div className="pt-5 border-t border-zinc-100 dark:border-zinc-800/80 grid grid-cols-1 sm:grid-cols-3 gap-4 text-xs">
                  <div className="flex items-center space-x-2.5">
                    <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400 flex-shrink-0" />
                    <div>
                      <span className="text-zinc-500 dark:text-zinc-400">DNS Sinkhole </span>
                      <strong className="text-zinc-800 dark:text-zinc-200 font-semibold font-mono">0.0.0.0 & ::</strong>
                    </div>
                  </div>
                  <div className="flex items-center space-x-2.5">
                    <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400 flex-shrink-0" />
                    <div>
                      <span className="text-zinc-500 dark:text-zinc-400">nftables IP Backstop </span>
                      <strong className="text-zinc-800 dark:text-zinc-200 font-semibold">Active</strong>
                    </div>
                  </div>
                  <div className="flex items-center space-x-2.5">
                    <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400 flex-shrink-0" />
                    <div>
                      <span className="text-zinc-500 dark:text-zinc-400">DoH/DoT Bypass </span>
                      <strong className="text-zinc-800 dark:text-zinc-200 font-semibold">Closed</strong>
                    </div>
                  </div>
                </div>
              </div>

              {/* Middle 3 Metric Cards */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                {/* Card 1: Enforced Policies */}
                <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-2xl p-5 shadow-[0_1px_3px_rgba(0,0,0,0.02)] flex flex-col justify-between space-y-4">
                  <div className="space-y-2">
                    <div className="flex items-center space-x-2 text-zinc-700 dark:text-zinc-300">
                      <ScrollText className="w-4 h-4 text-zinc-500 dark:text-zinc-400" />
                      <span className="text-xs font-semibold">Enforced Policies</span>
                    </div>
                    <div className="text-3xl font-bold text-zinc-900 dark:text-white">
                      {status?.policies?.length || 1}
                    </div>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400 leading-tight">
                      <p>1 System Locked</p>
                      <p>+ {customPolicyCount} Custom Rules</p>
                    </div>
                  </div>
                  <div className="flex justify-end pt-1">
                    <button
                      onClick={() => setTab('rules')}
                      className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-white dark:bg-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/80 text-xs font-medium text-zinc-700 dark:text-zinc-300 shadow-sm transition"
                    >
                      <span>View Policies</span>
                      <ArrowRight className="w-3 h-3 text-zinc-500" />
                    </button>
                  </div>
                </div>

                {/* Card 2: Sinkholed Domains */}
                <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-2xl p-5 shadow-[0_1px_3px_rgba(0,0,0,0.02)] flex flex-col justify-between space-y-4">
                  <div className="space-y-2">
                    <div className="flex items-center space-x-2 text-zinc-700 dark:text-zinc-300">
                      <Globe className="w-4 h-4 text-purple-600 dark:text-purple-400" />
                      <span className="text-xs font-semibold">Sinkholed Domains</span>
                    </div>
                    <div className="text-3xl font-bold text-zinc-900 dark:text-white">
                      {sinkholedDomainCount}
                    </div>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400 leading-tight">
                      <p>Zero-latency local</p>
                      <p>DNS interception</p>
                    </div>
                  </div>
                  <div className="flex justify-end pt-1">
                    <button
                      onClick={() => setActiveModal('domains')}
                      className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-white dark:bg-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/80 text-xs font-medium text-zinc-700 dark:text-zinc-300 shadow-sm transition"
                    >
                      <span>View Domains</span>
                      <ArrowRight className="w-3 h-3 text-zinc-500" />
                    </button>
                  </div>
                </div>

                {/* Card 3: Fail-Closed Guarantee */}
                <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-2xl p-5 shadow-[0_1px_3px_rgba(0,0,0,0.02)] flex flex-col justify-between space-y-4">
                  <div className="space-y-2">
                    <div className="flex items-center space-x-2 text-zinc-700 dark:text-zinc-300">
                      <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                      <span className="text-xs font-semibold">Fail-Closed Guarantee</span>
                    </div>
                    <div className="text-2xl font-bold text-emerald-600 dark:text-emerald-400">
                      Enforced
                    </div>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400 leading-tight">
                      <p>Daemon crashes retain</p>
                      <p>firewall blocks</p>
                    </div>
                  </div>
                  <div className="flex justify-end pt-1">
                    <button
                      onClick={() => setActiveModal('fail_closed')}
                      className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-white dark:bg-zinc-900 hover:bg-zinc-50 dark:hover:bg-zinc-800 border border-zinc-200 dark:border-zinc-700/80 text-xs font-medium text-zinc-700 dark:text-zinc-300 shadow-sm transition"
                    >
                      <span>View Details</span>
                      <ArrowRight className="w-3 h-3 text-zinc-500" />
                    </button>
                  </div>
                </div>
              </div>

              {/* Bottom Row Container (3 sections: Philosophy, Blocked Requests Chart, Top Blocked Domains) */}
              <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-2xl p-5 shadow-[0_1px_3px_rgba(0,0,0,0.02)] grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
                {/* Left: FocusWall in Action */}
                <div className="lg:col-span-4 space-y-3 pr-2">
                  <div className="flex items-center space-x-2">
                    <Activity className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                    <span className="text-xs font-semibold text-zinc-800 dark:text-zinc-200">FocusWall in Action</span>
                  </div>
                  <div className="text-xs text-zinc-500 dark:text-zinc-400 space-y-2 leading-relaxed">
                    <p>FocusWall operates on deliberate friction.</p>
                    <p>There is no pause toggle, no bypass password, and no instant delete.</p>
                    <p>Custom websites require a 24-hour waiting period before removal to prevent impulsive dismantling.</p>
                  </div>
                </div>

                {/* Middle: Blocked Requests (24h) Chart */}
                <div className="lg:col-span-5 space-y-3 lg:border-l lg:border-r border-zinc-100 dark:border-zinc-800 lg:px-4">
                  <div className="text-xs font-semibold text-zinc-800 dark:text-zinc-200">
                    Blocked Requests (24h)
                  </div>
                  <div className="flex items-end space-x-2 pt-1">
                    {/* Y-Axis labels */}
                    <div className="flex flex-col justify-between text-[10px] text-zinc-400 font-mono h-24 pb-1 text-right pr-1 select-none">
                      <span>120</span>
                      <span>80</span>
                      <span>40</span>
                      <span>0</span>
                    </div>

                    {/* Chart Bars */}
                    <div className="flex-1 flex flex-col">
                      <div className="h-24 flex items-end justify-between gap-1 pb-1 border-b border-zinc-200/80 dark:border-zinc-700/80">
                        {hourlyBlockedData.map((d, idx) => {
                          const heightPct = Math.min(100, Math.max(8, (d.count / 120) * 100));
                          return (
                            <div
                              key={idx}
                              className="flex-1 bg-emerald-500/85 hover:bg-emerald-600 dark:bg-emerald-500 dark:hover:bg-emerald-400 rounded-t-sm transition-all relative group cursor-pointer"
                              style={{ height: `${heightPct}%` }}
                            >
                              {/* Tooltip on hover */}
                              <div className="absolute bottom-full mb-1 left-1/2 -translate-x-1/2 hidden group-hover:flex flex-col items-center z-30 pointer-events-none">
                                <div className="bg-zinc-900 text-white text-[10px] font-mono px-2 py-0.5 rounded shadow-md whitespace-nowrap">
                                  {d.hour}: {d.count} blocked
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>

                      {/* X-Axis labels */}
                      <div className="flex justify-between text-[10px] text-zinc-400 font-mono pt-1.5 select-none">
                        <span>00:00</span>
                        <span>04:00</span>
                        <span>08:00</span>
                        <span>12:00</span>
                        <span>16:00</span>
                        <span>20:00</span>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Right: Top Blocked Domains */}
                <div className="lg:col-span-3 space-y-3 flex flex-col justify-between h-full">
                  <div className="space-y-2.5">
                    <div className="text-xs font-semibold text-zinc-800 dark:text-zinc-200">
                      Top Blocked Domains
                    </div>
                    <div className="space-y-1.5 text-xs">
                      <div className="flex justify-between items-center text-zinc-700 dark:text-zinc-300">
                        <span className="font-mono text-[11px] truncate max-w-[140px]">youtube.com</span>
                        <span className="font-mono text-[11px] text-zinc-500">128</span>
                      </div>
                      <div className="flex justify-between items-center text-zinc-700 dark:text-zinc-300">
                        <span className="font-mono text-[11px] truncate max-w-[140px]">www.youtube.com</span>
                        <span className="font-mono text-[11px] text-zinc-500">96</span>
                      </div>
                      <div className="flex justify-between items-center text-zinc-700 dark:text-zinc-300">
                        <span className="font-mono text-[11px] truncate max-w-[140px]">i.ytimg.com</span>
                        <span className="font-mono text-[11px] text-zinc-500">64</span>
                      </div>
                      <div className="flex justify-between items-center text-zinc-700 dark:text-zinc-300">
                        <span className="font-mono text-[11px] truncate max-w-[140px]">m.youtube.com</span>
                        <span className="font-mono text-[11px] text-zinc-500">32</span>
                      </div>
                    </div>
                  </div>

                  <div className="flex justify-end pt-2">
                    <button
                      onClick={() => setActiveModal('top_domains')}
                      className="inline-flex items-center space-x-1 text-xs text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-white font-medium transition"
                    >
                      <span>View All</span>
                      <ArrowRight className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              </div>

              {/* Bottom Footer Status Bar */}
              <div className="flex flex-wrap items-center justify-between gap-4 pt-3 text-xs text-zinc-500 dark:text-zinc-400">
                <div className="flex items-center space-x-2">
                  <Shield className="w-4 h-4 text-zinc-400" />
                  <span>Kernel Module: <strong className="text-zinc-700 dark:text-zinc-300 font-medium">Active</strong></span>
                </div>
                <div className="flex items-center space-x-2">
                  <Clock className="w-4 h-4 text-zinc-400" />
                  <span>Uptime: <strong className="text-zinc-700 dark:text-zinc-300 font-medium">5d 14h 22m</strong></span>
                </div>
                <div className="flex items-center space-x-2">
                  <RefreshCw className="w-4 h-4 text-zinc-400" />
                  <span>Last Updated: <strong className="text-zinc-700 dark:text-zinc-300 font-medium">Just now</strong></span>
                </div>
                <div className="flex items-center space-x-2">
                  <Info className="w-4 h-4 text-zinc-400" />
                  <span>Version: <strong className="text-zinc-700 dark:text-zinc-300 font-mono font-medium">v1.0.0</strong></span>
                </div>
              </div>
            </div>
          )}

          {/* TAB 2: BLOCKED WEBSITES */}
          {tab === 'rules' && (
            <div className="space-y-4 animate-in fade-in duration-150">
              {/* Header and Search */}
              <div className="flex flex-col sm:flex-row justify-between items-stretch sm:items-center gap-3">
                <div className="relative flex-1 max-w-md">
                  <Search className="w-4 h-4 text-zinc-400 absolute left-3.5 top-1/2 -translate-y-1/2" />
                  <input
                    type="text"
                    value={ruleSearch}
                    onChange={(e) => setRuleSearch(e.target.value)}
                    placeholder="Search blocked domains or policies..."
                    className="w-full pl-9 pr-4 py-2 rounded-xl bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 text-xs text-zinc-900 dark:text-white placeholder-zinc-400 focus:outline-none focus:border-emerald-500 transition shadow-sm"
                  />
                </div>

                <button
                  onClick={() => setTab('add')}
                  className="flex items-center justify-center space-x-2 px-4 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold shadow-sm transition"
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
                      className="p-5 rounded-2xl bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 shadow-sm flex flex-col md:flex-row md:items-center justify-between gap-4 transition"
                    >
                      <div className="space-y-2">
                        <div className="flex items-center space-x-2.5">
                          <div className="w-7 h-7 rounded-lg bg-zinc-100 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 flex items-center justify-center font-bold text-xs text-zinc-700 dark:text-zinc-200 uppercase">
                            {p.name.charAt(0)}
                          </div>
                          <span className="font-bold text-sm text-zinc-900 dark:text-white">{p.name}</span>
                          {isSystem ? (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-amber-50 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/25 text-amber-700 dark:text-amber-400 flex items-center space-x-1">
                              <Lock className="w-3 h-3 inline" />
                              <span>System Locked</span>
                            </span>
                          ) : (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-blue-50 dark:bg-blue-500/10 border border-blue-200 dark:border-blue-500/25 text-blue-700 dark:text-blue-400">
                              Custom Rule
                            </span>
                          )}
                          {isPending && (
                            <span className="text-[10px] uppercase font-mono font-semibold px-2 py-0.5 rounded-full bg-amber-100 dark:bg-amber-500/15 border border-amber-300 dark:border-amber-500/30 text-amber-800 dark:text-amber-300 animate-pulse">
                              Removal Cooldown Active
                            </span>
                          )}
                        </div>

                        <div className="text-xs text-zinc-500 dark:text-zinc-400 space-y-1">
                          <p>
                            Schedule: <span className="text-zinc-800 dark:text-zinc-200 font-medium">{p.schedule ? `Allowed ${p.schedule.start} – ${p.schedule.end} daily` : '24/7 Blocked'}</span>
                          </p>
                          <p className="font-mono text-[11px] text-zinc-500 dark:text-zinc-400">
                            Domains ({p.domains.length}): {p.domains.join(', ')}
                          </p>
                          {isPending && p.earliest_removal_at && (
                            <p className="text-amber-600 dark:text-amber-400 font-mono text-[11px] pt-1">
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
                              className="px-3 py-1.5 rounded-xl bg-zinc-100 hover:bg-zinc-200 dark:bg-zinc-800 dark:hover:bg-zinc-700 text-xs font-medium text-zinc-700 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-700 transition"
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
                            className="px-3 py-1.5 rounded-xl bg-zinc-50 hover:bg-red-50 hover:border-red-200 hover:text-red-600 dark:bg-zinc-900 dark:hover:bg-red-500/10 dark:hover:border-red-500/30 dark:hover:text-red-400 border border-zinc-200 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400 text-xs font-medium transition"
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
            <div className="max-w-lg mx-auto w-full space-y-6 pt-2 animate-in fade-in duration-150">
              <div className="text-center space-y-1.5">
                <h2 className="text-xl font-bold text-zinc-900 dark:text-white tracking-tight">Block Website</h2>
                <p className="text-xs text-zinc-500 dark:text-zinc-400">
                  Enter any domain or link. It will automatically normalize to its registrable root domain.
                </p>
              </div>

              <div className="p-6 rounded-3xl bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 shadow-sm space-y-5">
                <div>
                  <label className="block text-xs font-semibold text-zinc-700 dark:text-zinc-300 mb-1.5">Website Domain / Link</label>
                  <input
                    type="text"
                    value={rawInput}
                    onChange={(e) => setRawInput(e.target.value)}
                    placeholder="e.g. reddit.com, twitter.com, news.ycombinator.com"
                    autoFocus
                    className="w-full px-4 py-3 rounded-xl bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 text-sm text-zinc-900 dark:text-white placeholder-zinc-400 focus:outline-none focus:border-emerald-500 transition font-mono"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-zinc-700 dark:text-zinc-300 mb-1.5">Removal Cooldown Duration</label>
                  <div className="grid grid-cols-3 gap-2">
                    {[24, 48, 72].map((hours) => (
                      <button
                        key={hours}
                        type="button"
                        onClick={() => setCooldownHours(hours)}
                        className={`py-2.5 rounded-xl text-xs font-medium border transition ${
                          cooldownHours === hours
                            ? 'bg-emerald-50 dark:bg-emerald-500/10 border-emerald-300 dark:border-emerald-500/40 text-emerald-700 dark:text-emerald-400 font-semibold'
                            : 'bg-zinc-50 dark:bg-zinc-950 border-zinc-200 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200'
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
                  <div className="p-4 rounded-2xl bg-emerald-50 dark:bg-emerald-500/10 border border-emerald-200 dark:border-emerald-500/25 space-y-2">
                    <div className="flex items-center space-x-2 text-emerald-700 dark:text-emerald-400 font-semibold text-xs">
                      <CheckCircle className="w-4 h-4" />
                      <span>Normalization Preview</span>
                    </div>
                    <div className="text-xs text-zinc-700 dark:text-zinc-300 space-y-1">
                      <p>Registrable Root: <strong className="text-zinc-900 dark:text-white font-mono">{preview.root_domain}</strong></p>
                      <p>Generated Patterns: <code className="text-emerald-700 dark:text-emerald-300 font-mono text-[11px]">{preview.domains.join(', ')}</code></p>
                    </div>
                  </div>
                )}

                <button
                  disabled={!preview || actionLoading}
                  onClick={() => setShowConfirmModal(true)}
                  className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-40 text-white font-semibold text-xs transition shadow-sm flex justify-center items-center space-x-2"
                >
                  <span>Review & Enforce Block</span>
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>

              {/* Confirmation Modal */}
              {showConfirmModal && preview && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
                  <div className="bg-white dark:bg-[#121216] border border-zinc-200 dark:border-zinc-800 max-w-md w-full rounded-3xl p-6 space-y-5 shadow-2xl">
                    <div className="flex justify-between items-start">
                      <div className="space-y-1">
                        <h4 className="text-base font-bold text-zinc-900 dark:text-white flex items-center space-x-2">
                          <AlertCircle className="w-5 h-5 text-amber-500" />
                          <span>Confirm Permanent Block</span>
                        </h4>
                        <p className="text-xs text-zinc-500 dark:text-zinc-400">
                          Confirming will enforce 24/7 system-level DNS sinkholing on:
                        </p>
                      </div>
                      <button
                        onClick={() => setShowConfirmModal(false)}
                        className="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-1 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>

                    <div className="p-4 bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800/80 rounded-2xl text-xs space-y-2">
                      <div className="text-zinc-500 dark:text-zinc-400 font-medium">Domain Scope:</div>
                      <ul className="list-disc list-inside text-zinc-800 dark:text-zinc-200 font-mono text-[11px] space-y-0.5">
                        {preview.domains.map(d => <li key={d}>{d}</li>)}
                      </ul>
                      <div className="pt-2 text-zinc-500 dark:text-zinc-400 border-t border-zinc-200 dark:border-zinc-800 flex justify-between">
                        <span>Required Removal Cooldown:</span>
                        <strong className="text-amber-600 dark:text-amber-400 font-mono">{cooldownHours} hours</strong>
                      </div>
                    </div>

                    <div className="flex space-x-3">
                      <button
                        onClick={() => setShowConfirmModal(false)}
                        className="flex-1 py-2.5 rounded-xl bg-zinc-100 hover:bg-zinc-200 dark:bg-zinc-800 dark:hover:bg-zinc-700 text-xs font-semibold text-zinc-700 dark:text-zinc-300 transition"
                      >
                        Cancel
                      </button>
                      <button
                        disabled={actionLoading}
                        onClick={handleAddRule}
                        className="flex-1 py-2.5 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-xs font-bold text-white transition shadow-sm"
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
            <div className="space-y-4 animate-in fade-in duration-150">
              <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-3">
                <div className="flex items-center space-x-2">
                  {['all', 'daemon', 'policy', 'removal', 'dns'].map((filter) => (
                    <button
                      key={filter}
                      onClick={() => setLogFilter(filter)}
                      className={`px-3 py-1 rounded-lg text-xs font-medium capitalize transition ${
                        logFilter === filter
                          ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900'
                          : 'text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
                      }`}
                    >
                      {filter}
                    </button>
                  ))}
                </div>

                <span className="text-xs text-zinc-500 dark:text-zinc-400 font-mono">
                  {filteredLogs.length} events recorded
                </span>
              </div>

              <div className="border border-zinc-200 dark:border-zinc-800 rounded-2xl overflow-hidden bg-white dark:bg-zinc-950/50 shadow-sm">
                <table className="w-full text-left text-xs">
                  <thead className="bg-zinc-50 dark:bg-zinc-900/60 border-b border-zinc-200 dark:border-zinc-800 text-zinc-500 dark:text-zinc-400 font-mono text-[11px]">
                    <tr>
                      <th className="p-3.5 font-semibold">Timestamp</th>
                      <th className="p-3.5 font-semibold">Event</th>
                      <th className="p-3.5 font-semibold">Event Details</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-zinc-100 dark:divide-zinc-800/50 font-mono">
                    {filteredLogs.map((log) => (
                      <tr key={log.id} className="hover:bg-zinc-50/80 dark:hover:bg-zinc-900/30 transition">
                        <td className="p-3.5 text-zinc-500 dark:text-zinc-400 whitespace-nowrap text-[11px]">{new Date(log.ts).toLocaleString()}</td>
                        <td className="p-3.5 whitespace-nowrap">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase ${
                            log.event_type.includes('start') ? 'bg-blue-50 text-blue-700 dark:bg-blue-500/10 dark:text-blue-400 border border-blue-200 dark:border-blue-500/20' :
                            log.event_type.includes('requested') ? 'bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:text-amber-400 border border-amber-200 dark:border-amber-500/20' :
                            log.event_type.includes('confirmed') ? 'bg-red-50 text-red-700 dark:bg-red-500/10 dark:text-red-400 border border-red-200 dark:border-red-500/20' :
                            'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/10 dark:text-emerald-400 border border-emerald-200 dark:border-emerald-500/20'
                          }`}>
                            {log.event_type}
                          </span>
                        </td>
                        <td className="p-3.5 text-zinc-800 dark:text-zinc-300 font-sans text-xs">{log.detail}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* TAB 5: SETTINGS */}
          {tab === 'settings' && (
            <div className="max-w-2xl mx-auto space-y-6 pt-2 animate-in fade-in duration-150">
              <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-3xl p-6 shadow-sm space-y-6">
                <h3 className="text-base font-bold text-zinc-900 dark:text-white">Appearance & Theme</h3>
                <div className="grid grid-cols-2 gap-3">
                  <button
                    onClick={() => setIsDarkMode(false)}
                    className={`p-4 rounded-2xl border text-left flex items-center justify-between transition ${
                      !isDarkMode
                        ? 'border-emerald-500 bg-emerald-50/50 dark:bg-emerald-500/10 text-emerald-900 dark:text-white font-semibold'
                        : 'border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400'
                    }`}
                  >
                    <div className="flex items-center space-x-3">
                      <Sun className="w-5 h-5 text-amber-500" />
                      <div>
                        <div className="text-xs font-bold">Light Mode</div>
                        <div className="text-[11px] text-zinc-500">Matches reference design</div>
                      </div>
                    </div>
                    {!isDarkMode && <Check className="w-4 h-4 text-emerald-600" />}
                  </button>

                  <button
                    onClick={() => setIsDarkMode(true)}
                    className={`p-4 rounded-2xl border text-left flex items-center justify-between transition ${
                      isDarkMode
                        ? 'border-emerald-500 bg-emerald-50/50 dark:bg-emerald-500/10 text-emerald-900 dark:text-white font-semibold'
                        : 'border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400'
                    }`}
                  >
                    <div className="flex items-center space-x-3">
                      <Moon className="w-5 h-5 text-indigo-500" />
                      <div>
                        <div className="text-xs font-bold">Dark Mode</div>
                        <div className="text-[11px] text-zinc-500">Dark aesthetic for low light</div>
                      </div>
                    </div>
                    {isDarkMode && <Check className="w-4 h-4 text-emerald-600" />}
                  </button>
                </div>
              </div>

              <div className="bg-white dark:bg-[#121216] border border-zinc-200/90 dark:border-zinc-800/90 rounded-3xl p-6 shadow-sm space-y-4">
                <h3 className="text-base font-bold text-zinc-900 dark:text-white">Kernel & Firewall Status</h3>
                <div className="space-y-3 text-xs">
                  <div className="flex justify-between items-center py-2 border-b border-zinc-100 dark:border-zinc-800">
                    <span className="text-zinc-600 dark:text-zinc-400">nftables table</span>
                    <span className="font-mono text-zinc-900 dark:text-white font-medium">inet focuswall</span>
                  </div>
                  <div className="flex justify-between items-center py-2 border-b border-zinc-100 dark:border-zinc-800">
                    <span className="text-zinc-600 dark:text-zinc-400">DNS Sinkhole Socket</span>
                    <span className="font-mono text-zinc-900 dark:text-white font-medium">127.0.0.53:53</span>
                  </div>
                  <div className="flex justify-between items-center py-2 border-b border-zinc-100 dark:border-zinc-800">
                    <span className="text-zinc-600 dark:text-zinc-400">IPC Socket Path</span>
                    <span className="font-mono text-zinc-900 dark:text-white font-medium">/run/focuswalld/focuswall.sock</span>
                  </div>
                  <div className="flex justify-between items-center py-2">
                    <span className="text-zinc-600 dark:text-zinc-400">Fail-Closed Policy Guarantee</span>
                    <span className="text-emerald-600 dark:text-emerald-400 font-bold">STRICT_KERNEL_PERSIST</span>
                  </div>
                </div>
              </div>
            </div>
          )}

        </div>
      </main>

      {/* POPUP MODAL 1: SINKHOLED DOMAINS */}
      {activeModal === 'domains' && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="bg-white dark:bg-[#121216] border border-zinc-200 dark:border-zinc-800 max-w-lg w-full rounded-3xl p-6 space-y-4 shadow-2xl">
            <div className="flex justify-between items-center">
              <div className="flex items-center space-x-2">
                <Globe className="w-5 h-5 text-purple-600 dark:text-purple-400" />
                <h4 className="text-base font-bold text-zinc-900 dark:text-white">Active Sinkholed Domains</h4>
              </div>
              <button
                onClick={() => setActiveModal(null)}
                className="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-1 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <p className="text-xs text-zinc-500 dark:text-zinc-400">
              All listed domains are resolved locally to <code className="font-mono text-emerald-600 dark:text-emerald-400">0.0.0.0</code> and <code className="font-mono text-emerald-600 dark:text-emerald-400">::</code> with 0ms latency.
            </p>
            <div className="bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-2xl p-4 max-h-60 overflow-y-auto space-y-2 font-mono text-xs">
              {(status?.blocked_domains || ['youtube.com', 'www.youtube.com', 'i.ytimg.com', 'm.youtube.com']).map((domain) => (
                <div key={domain} className="flex justify-between items-center py-1 border-b border-zinc-200/60 dark:border-zinc-800/60 last:border-0">
                  <span className="text-zinc-800 dark:text-zinc-200">{domain}</span>
                  <span className="text-[10px] text-emerald-600 dark:text-emerald-400 font-bold bg-emerald-50 dark:bg-emerald-500/10 px-2 py-0.5 rounded">Sinkholed</span>
                </div>
              ))}
            </div>
            <div className="flex justify-end">
              <button
                onClick={() => setActiveModal(null)}
                className="px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 dark:bg-zinc-100 dark:hover:bg-white text-white dark:text-zinc-900 text-xs font-semibold transition"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}

      {/* POPUP MODAL 2: FAIL-CLOSED GUARANTEE DETAILS */}
      {activeModal === 'fail_closed' && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="bg-white dark:bg-[#121216] border border-zinc-200 dark:border-zinc-800 max-w-lg w-full rounded-3xl p-6 space-y-4 shadow-2xl">
            <div className="flex justify-between items-center">
              <div className="flex items-center space-x-2">
                <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
                <h4 className="text-base font-bold text-zinc-900 dark:text-white">Fail-Closed Guarantee Architecture</h4>
              </div>
              <button
                onClick={() => setActiveModal(null)}
                className="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-1 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="text-xs text-zinc-600 dark:text-zinc-300 space-y-3 leading-relaxed">
              <div className="p-3.5 bg-emerald-50 dark:bg-emerald-500/10 border border-emerald-200 dark:border-emerald-500/20 rounded-2xl text-emerald-900 dark:text-emerald-300">
                <strong>Why FocusWall is unbreakable:</strong> Unlike browser extensions or userland blockers, FocusWall loads its rule tables directly into Linux <code className="font-mono">nftables</code> and the root resolver table.
              </div>
              <ul className="list-disc list-inside space-y-1 text-zinc-500 dark:text-zinc-400 text-xs">
                <li>If the UI app is closed, kernel rules remain 100% active.</li>
                <li>If the daemon crashes or is SIGKILL'd, nftables retain strict DROP chains.</li>
                <li>DNS-over-HTTPS (DoH) and DNS-over-TLS (DoT) egress ports 853/443 to external resolvers are intercepted.</li>
                <li>Uninstallation requires entering a mandatory 24-hour verification hold.</li>
              </ul>
            </div>
            <div className="flex justify-end">
              <button
                onClick={() => setActiveModal(null)}
                className="px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 dark:bg-zinc-100 dark:hover:bg-white text-white dark:text-zinc-900 text-xs font-semibold transition"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {/* POPUP MODAL 3: TOP DOMAINS FULL LIST */}
      {activeModal === 'top_domains' && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="bg-white dark:bg-[#121216] border border-zinc-200 dark:border-zinc-800 max-w-lg w-full rounded-3xl p-6 space-y-4 shadow-2xl">
            <div className="flex justify-between items-center">
              <div className="flex items-center space-x-2">
                <Zap className="w-5 h-5 text-amber-500" />
                <h4 className="text-base font-bold text-zinc-900 dark:text-white">Top Blocked Domains (24h)</h4>
              </div>
              <button
                onClick={() => setActiveModal(null)}
                className="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-1 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-2xl p-4 max-h-60 overflow-y-auto space-y-2 text-xs">
              {[
                { domain: 'youtube.com', count: 128 },
                { domain: 'www.youtube.com', count: 96 },
                { domain: 'i.ytimg.com', count: 64 },
                { domain: 'm.youtube.com', count: 32 },
                { domain: 'music.youtube.com', count: 18 },
                { domain: 'googlevideo.com', count: 14 },
                { domain: 'youtu.be', count: 9 },
              ].map((item, idx) => (
                <div key={item.domain} className="flex justify-between items-center py-1.5 border-b border-zinc-200/60 dark:border-zinc-800/60 last:border-0">
                  <div className="flex items-center space-x-2 font-mono">
                    <span className="text-zinc-400 text-[10px] w-4">{idx + 1}.</span>
                    <span className="text-zinc-800 dark:text-zinc-200">{item.domain}</span>
                  </div>
                  <span className="font-mono text-zinc-600 dark:text-zinc-400 font-semibold">{item.count} hits</span>
                </div>
              ))}
            </div>
            <div className="flex justify-end">
              <button
                onClick={() => setActiveModal(null)}
                className="px-4 py-2 rounded-xl bg-zinc-900 hover:bg-zinc-800 dark:bg-zinc-100 dark:hover:bg-white text-white dark:text-zinc-900 text-xs font-semibold transition"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
