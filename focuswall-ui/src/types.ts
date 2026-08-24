export type BlockState = 'allowed' | 'blocked';
export type PolicyKind = 'system' | 'custom';
export type PolicyStatus = 'active' | 'removal_pending' | 'removed';

export interface TimeWindow {
  start: string;
  end: string;
}

export interface Policy {
  id?: number;
  kind: PolicyKind;
  name: string;
  domains: string[];
  schedule?: TimeWindow;
  timezone: string;
  status: PolicyStatus;
  created_at: string;
  removal_requested_at?: string;
  removal_cooldown_hours?: number;
  earliest_removal_at?: string;
  removal_reason?: string;
}

export interface AuditLogEntry {
  id: number;
  ts: string;
  event_type: string;
  detail: string;
}

export interface NormalizedPreview {
  root_domain: string;
  domains: string[];
}

export interface SystemStatus {
  current_time: string;
  youtube_state: BlockState;
  policies: Policy[];
  blocked_domains: string[];
}
