export interface AuditEvent {
  at: number;
  kind: string;
  actor: string;
  detail: string;
}
export interface Session {
  id: string;
  operator: string;
  expires_at: number;
}
export interface Pending {
  id: string;
  operator: string;
  created_at: number;
}
export interface Host {
  running: boolean;
  url: string;
  local_url: string;
  working_directory: string;
  invite: string | null;
  invite_expires_at: number | null;
  pending: Pending[];
  session: Session | null;
  audit: AuditEvent[];
  tunnel: string;
  error: string | null;
}
export interface Remote {
  status: string;
  url: string;
  operator: string;
  expires_at: number | null;
  os: string | null;
  error: string | null;
}
export interface Overview {
  host: Host;
  remote: Remote;
  home: string;
  audit_path: string;
  executable: string;
}
export type Mode = 'host' | 'operator';
export type Page = 'workspace' | 'activity' | 'agents' | 'about';
