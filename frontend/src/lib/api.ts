export interface StatusResponse {
  version: string;
  uptime_seconds: number;
  dns_port: number;
  web_port: number;
  cache_entries: number;
  adblock_rules_count: number;
  anti_pollution: boolean;
}

export interface HourlyMetric {
  hour: string;
  total: number;
  blocked: number;
  avg_duration_ms: number;
}

export interface MetricsSnapshot {
  total_queries: number;
  blocked_queries: number;
  cached_queries: number;
  rewritten_queries: number;
  block_rate: number;
  cache_hit_rate: number;
  avg_duration_ms: number;
  qps: number;
  hourly_history?: HourlyMetric[];
  top_domains: [string, number][];
  top_blocked: [string, number][];
  top_clients: [string, number][];
}

export interface CacheStatRow {
  requests: number;
  hits: number;
  misses: number;
  hit_rate: number;
}

export interface CacheStatsSnapshot {
  yesterday: CacheStatRow;
  today: CacheStatRow;
  total: CacheStatRow;
  updated_at: string;
}

export interface QueryLogItem {
  id: string;
  timestamp: string;
  client_ip: string;
  client_name?: string;
  qname: string;
  qtype: string;
  status: string;
  rcode: string;
  matched_rule?: string;
  routing_group?: string;
  response_ip?: string;
  duration_ms: number;
}

export interface QueryLogResponse {
  items: QueryLogItem[];
  total: number;
  page: number;
  page_size: number;
}

export interface PipelineNode {
  id: string;
  name: string;
  stage: string;
  description: string;
  enabled: boolean;
}

export interface AdblockRuleRecord {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  rule_count: number;
  updated_at: string;
}

export interface UpstreamsResponse {
  bootstrap_dns: string[];
  mainland: string[];
  gfw: string[];
  overseas: string[];
}

export interface UpstreamTestItem {
  addr: string;
  group: string;
  latency_ms?: number;
  ok: boolean;
}

export interface DnsRewriteRecord {
  id: string;
  domain: string;
  record_type: string;
  value: string;
  enabled: boolean;
}

export interface RuleFileInfo {
  id: string;
  name: string;
  file_name: string;
  group_type: string;
  count: number;
  size_bytes: number;
  updated_at: string;
  url: string;
  enabled: boolean;
}

export interface AuthResponse {
  token: string;
  username: string;
  expires_at: string;
}

export interface ToolAnswerItem {
  record_type: string;
  data: string;
  ttl: number;
}

export interface ToolQueryResponse {
  domain: string;
  qtype: string;
  status: string;
  outcome: string;
  rcode: string;
  duration_ms: number;
  answers_count: number;
  routing_group?: string | null;
  matched_rule?: string | null;
  answers: ToolAnswerItem[];
  summary: string;
}

export interface ProxyItem {
  id: string;
  name: string;
  addr: string;
  is_default: boolean;
}

export interface ProxyConfig {
  enabled: boolean;
  socks5?: string;
  list: ProxyItem[];
}

export interface AppSettings {
  dns: {
    enabled: boolean;
    port: number;
    listen_mode: string;
    connect_ip_version?: string;
    response_mode: string;
    doh_enabled: boolean;
    doh_port: number;
    doh_path: string;
    block_type: string;
    block_ttl: number;
  };
  web: {
    port: number;
  };
  proxy: ProxyConfig;
  cache: {
    size: number;
    prefetch: boolean;
    min_ttl: number;
    max_ttl: number;
  };
  speed_check: {
    enabled: boolean;
    timeout_ms: number;
    cache_mins: number;
    mode: string;
  };
  anti_pollution: boolean;
  routing: {
    enabled?: boolean;
    anti_pollution: boolean;
    fallback_timeout_ms: number;
  };
  logging: {
    enabled: boolean;
    retention_mins: number;
    retention_days?: number;
  };
  rules_update?: {
    auto_update: boolean;
    interval_mins: number;
  };
  upstreams: {
    bootstrap_dns?: string[];
    mainland: string[];
    gfw: string[];
    overseas: string[];
    timeout_ms: number;
  };
}

export async function authFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const token = localStorage.getItem('nexdns_token');
  const headers = new Headers(init?.headers || {});
  if (!headers.has('Content-Type') && !(init?.body instanceof FormData)) {
    headers.set('Content-Type', 'application/json');
  }
  if (token) {
    headers.set('Authorization', `Bearer ${token}`);
  }
  const res = await fetch(input, { ...init, headers });
  if (res.status === 401) {
    const urlStr = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
    if (!urlStr.includes('/api/v1/auth/login') && !urlStr.includes('/api/v1/auth/me')) {
      localStorage.removeItem('nexdns_token');
      localStorage.removeItem('nexdns_user');
      window.location.reload();
    }
  }
  return res;
}

function getAuthHeaders(): HeadersInit {
  const token = localStorage.getItem('nexdns_token');
  const h: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) h['Authorization'] = `Bearer ${token}`;
  return h;
}

export const api = {
  async login(username: string, password: string): Promise<AuthResponse> {
    const res = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || '登录失败');
    }
    return res.json();
  },

  async logout(): Promise<void> {
    const token = localStorage.getItem('nexdns_token') || '';
    await fetch('/api/v1/auth/logout', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token }),
    });
    localStorage.removeItem('nexdns_token');
    localStorage.removeItem('nexdns_user');
  },

  async getMe(): Promise<{ username: string; authenticated: boolean }> {
    const res = await authFetch('/api/v1/auth/me', {
      headers: getAuthHeaders(),
    });
    return res.json();
  },

  async changePassword(old_password: string, new_password: string): Promise<void> {
    const username = localStorage.getItem('nexdns_user') || 'admin';
    const res = await authFetch('/api/v1/auth/password', {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify({ username, old_password, new_password }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || '修改密码失败');
    }
  },

  async getSettings(): Promise<AppSettings> {
    const res = await authFetch('/api/v1/settings', {
      headers: getAuthHeaders(),
    });
    return res.json();
  },

  async updateSettings(settings: AppSettings): Promise<void> {
    const res = await authFetch('/api/v1/settings', {
      method: 'PUT',
      headers: getAuthHeaders(),
      body: JSON.stringify(settings),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || '保存配置失败');
    }
  },

  async testProxy(socks5: string): Promise<{ ok: boolean; latency_ms?: number; error?: string }> {
    const res = await authFetch('/api/v1/proxy/test', {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify({ socks5 }),
    });
    return res.json();
  },
  async getStatus(): Promise<StatusResponse> {
    const res = await authFetch('/api/v1/status');
    return res.json();
  },

  async getMetrics(): Promise<MetricsSnapshot> {
    const res = await authFetch('/api/v1/metrics');
    return res.json();
  },

  async getCacheStats(): Promise<CacheStatsSnapshot> {
    const res = await authFetch('/api/v1/metrics/cache-stats');
    if (!res.ok) throw new Error('Failed to fetch cache stats');
    return res.json();
  },

  async clearCacheStats(): Promise<void> {
    const res = await authFetch('/api/v1/metrics/cache-stats/clear', { method: 'POST' });
    if (!res.ok) throw new Error('Failed to clear cache stats');
  },

  async getQueryLogs(params: {
    page?: number;
    page_size?: number;
    search?: string;
    status?: string;
    client?: string;
  }): Promise<QueryLogResponse> {
    const sp = new URLSearchParams();
    if (params.page) sp.set('page', params.page.toString());
    if (params.page_size) sp.set('page_size', params.page_size.toString());
    if (params.search) sp.set('search', params.search);
    if (params.status && params.status !== 'all') sp.set('status', params.status);
    if (params.client) sp.set('client', params.client);
    const res = await authFetch(`/api/v1/querylog?${sp.toString()}`);
    return res.json();
  },

  async clearQueryLogs(): Promise<void> {
    await authFetch('/api/v1/querylog/clear', { method: 'POST' });
  },

  async getPipeline(): Promise<PipelineNode[]> {
    const res = await authFetch('/api/v1/pipeline');
    return res.json();
  },

  async getRules(): Promise<AdblockRuleRecord[]> {
    const res = await authFetch('/api/v1/rules');
    return res.json();
  },

  async addRule(payload: { name: string; url: string; content?: string }): Promise<void> {
    await authFetch('/api/v1/rules', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async deleteRule(id: string): Promise<void> {
    await authFetch(`/api/v1/rules/${id}`, { method: 'DELETE' });
  },

  async refreshRule(id: string): Promise<void> {
    await authFetch(`/api/v1/rules/${id}/refresh`, { method: 'POST' });
  },

  async testRule(domain: string): Promise<{ domain: string; blocked: boolean; matched_rule?: string }> {
    const res = await authFetch('/api/v1/rules/test', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain }),
    });
    return res.json();
  },

  async getUpstreams(): Promise<UpstreamsResponse> {
    const res = await authFetch('/api/v1/upstreams');
    return res.json();
  },

  async testUpstreams(): Promise<UpstreamTestItem[]> {
    const res = await authFetch('/api/v1/upstreams/test', { method: 'POST' });
    return res.json();
  },

  async testGroupUpstreams(group: string): Promise<UpstreamTestItem[]> {
    const res = await authFetch('/api/v1/upstreams/test-group', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ group }),
    });
    if (!res.ok) throw new Error('分组测速失败');
    return res.json();
  },

  async getRewrites(): Promise<DnsRewriteRecord[]> {
    const res = await authFetch('/api/v1/rewrites');
    return res.json();
  },

  async addRewrite(payload: { domain: string; record_type: string; value: string }): Promise<void> {
    await authFetch('/api/v1/rewrites', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async deleteRewrite(id: string): Promise<void> {
    await authFetch(`/api/v1/rewrites/${id}`, { method: 'DELETE' });
  },

  async flushCache(): Promise<void> {
    await authFetch('/api/v1/cache/flush', { method: 'POST' });
  },
  async togglePipelineStage(id: string, enabled: boolean): Promise<void> {
    const res = await authFetch('/api/v1/pipeline/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, enabled }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || '切换流水线节点状态失败');
    }
  },

  async toggleRule(id: string): Promise<void> {
    await authFetch(`/api/v1/rules/${id}/toggle`, { method: 'POST' });
  },

  async quickAddRule(domain: string, action: 'block' | 'whitelist'): Promise<void> {
    await authFetch('/api/v1/rules/quick-add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain, action }),
    });
  },

  async getRoutingRules(): Promise<RuleFileInfo[]> {
    const res = await authFetch('/api/v1/routing/rules');
    return res.json();
  },

  async updateRoutingRules(): Promise<[string, { Ok?: number; Err?: string }][]> {
    const res = await authFetch('/api/v1/routing/rules/update', { method: 'POST' });
    return res.json();
  },

  async addRoutingRule(payload: {
    name: string;
    file_name: string;
    group_type: string;
    url?: string;
    content?: string;
  }): Promise<void> {
    await authFetch('/api/v1/routing/rules/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async deleteRoutingRule(id: string): Promise<void> {
    await authFetch(`/api/v1/routing/rules/${id}`, { method: 'DELETE' });
  },

  async toggleRoutingRule(id: string): Promise<void> {
    await authFetch(`/api/v1/routing/rules/${id}/toggle`, { method: 'POST' });
  },

  async refreshRoutingRule(id: string): Promise<void> {
    await authFetch(`/api/v1/routing/rules/${id}/refresh`, { method: 'POST' });
  },

  async editRoutingRuleInfo(id: string, payload: { name: string; group_type: string; url: string }): Promise<void> {
    const res = await authFetch(`/api/v1/routing/rules/${id}/edit`, {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      throw new Error(await res.text());
    }
  },

  async addUpstream(group: string, addr: string): Promise<void> {
    await authFetch('/api/v1/upstreams/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ group, addr }),
    });
  },

  async editUpstream(payload: { old_group: string; old_addr: string; new_group: string; new_addr: string }): Promise<void> {
    const res = await authFetch('/api/v1/upstreams/edit', {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify(payload),
    });
    if (!res.ok) {
      throw new Error(await res.text());
    }
  },

  async reorderUpstreams(group: string, addrs: string[]): Promise<void> {
    const res = await authFetch('/api/v1/upstreams/reorder', {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify({ group, addrs }),
    });
    if (!res.ok) {
      throw new Error(await res.text());
    }
  },

  async testSingleUpstream(group: string, addr: string): Promise<{ ok: boolean; latency_ms?: number; error?: string }> {
    const res = await authFetch('/api/v1/upstreams/test-single', {
      method: 'POST',
      headers: getAuthHeaders(),
      body: JSON.stringify({ group, addr }),
    });
    return res.json();
  },

  async removeUpstream(group: string, addr: string): Promise<void> {
    await authFetch('/api/v1/upstreams/remove', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ group, addr }),
    });
  },

  async updateBootstrapDns(dns_list: string[]): Promise<void> {
    const res = await authFetch('/api/v1/upstreams/bootstrap', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ dns_list }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || '更新引导 DNS 失败');
    }
  },
  async getRuleContent(id: string): Promise<{ id: string; content: string }> {
    const res = await authFetch(`/api/v1/rules/${id}/content`);
    return res.json();
  },

  async updateRuleContent(id: string, content: string): Promise<void> {
    await authFetch(`/api/v1/rules/${id}/content`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
  },

  async getRoutingRuleContent(fileName: string): Promise<{ file_name: string; content: string }> {
    const res = await authFetch(`/api/v1/routing/rules/${fileName}/content`);
    return res.json();
  },

  async updateRoutingRuleContent(fileName: string, content: string): Promise<void> {
    await authFetch(`/api/v1/routing/rules/${fileName}/content`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
  },

  async getRawConfig(): Promise<{ content: string }> {
    const res = await authFetch('/api/v1/config/raw');
    return res.json();
  },

  async updateRawConfig(content: string): Promise<void> {
    const res = await authFetch('/api/v1/config/raw', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err);
    }
  },

  async toolQuery(domain: string, qtype: string = 'A'): Promise<ToolQueryResponse> {
    const res = await authFetch('/api/v1/tools/query', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain, qtype }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err || 'DNS 查询测试失败');
    }
    return res.json();
  },
};
