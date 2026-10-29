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
  name: string;
  file_name: string;
  count: number;
  size_bytes: number;
  updated_at: string;
  url: string;
}

export const api = {
  async getStatus(): Promise<StatusResponse> {
    const res = await fetch('/api/v1/status');
    return res.json();
  },

  async getMetrics(): Promise<MetricsSnapshot> {
    const res = await fetch('/api/v1/metrics');
    return res.json();
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
    const res = await fetch(`/api/v1/querylog?${sp.toString()}`);
    return res.json();
  },

  async clearQueryLogs(): Promise<void> {
    await fetch('/api/v1/querylog/clear', { method: 'POST' });
  },

  async getPipeline(): Promise<PipelineNode[]> {
    const res = await fetch('/api/v1/pipeline');
    return res.json();
  },

  async getRules(): Promise<AdblockRuleRecord[]> {
    const res = await fetch('/api/v1/rules');
    return res.json();
  },

  async addRule(payload: { name: string; url: string; content?: string }): Promise<void> {
    await fetch('/api/v1/rules', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async deleteRule(id: string): Promise<void> {
    await fetch(`/api/v1/rules/${id}`, { method: 'DELETE' });
  },

  async refreshRule(id: string): Promise<void> {
    await fetch(`/api/v1/rules/${id}/refresh`, { method: 'POST' });
  },

  async testRule(domain: string): Promise<{ domain: string; blocked: boolean; matched_rule?: string }> {
    const res = await fetch('/api/v1/rules/test', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain }),
    });
    return res.json();
  },

  async getUpstreams(): Promise<UpstreamsResponse> {
    const res = await fetch('/api/v1/upstreams');
    return res.json();
  },

  async testUpstreams(): Promise<UpstreamTestItem[]> {
    const res = await fetch('/api/v1/upstreams/test', { method: 'POST' });
    return res.json();
  },

  async getRewrites(): Promise<DnsRewriteRecord[]> {
    const res = await fetch('/api/v1/rewrites');
    return res.json();
  },

  async addRewrite(payload: { domain: string; record_type: string; value: string }): Promise<void> {
    await fetch('/api/v1/rewrites', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async deleteRewrite(id: string): Promise<void> {
    await fetch(`/api/v1/rewrites/${id}`, { method: 'DELETE' });
  },

  async flushCache(): Promise<void> {
    await fetch('/api/v1/cache/flush', { method: 'POST' });
  },
  async togglePipelineStage(id: string, enabled: boolean): Promise<void> {
    await fetch('/api/v1/pipeline/toggle', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, enabled }),
    });
  },

  async toggleRule(id: string): Promise<void> {
    await fetch(`/api/v1/rules/${id}/toggle`, { method: 'POST' });
  },

  async quickAddRule(domain: string, action: 'block' | 'whitelist'): Promise<void> {
    await fetch('/api/v1/rules/quick-add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain, action }),
    });
  },

  async getRoutingRules(): Promise<RuleFileInfo[]> {
    const res = await fetch('/api/v1/routing/rules');
    return res.json();
  },

  async updateRoutingRules(): Promise<[string, { Ok?: number; Err?: string }][]> {
    const res = await fetch('/api/v1/routing/rules/update', { method: 'POST' });
    return res.json();
  },

  async addUpstream(group: string, addr: string): Promise<void> {
    await fetch('/api/v1/upstreams/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ group, addr }),
    });
  },

  async removeUpstream(group: string, addr: string): Promise<void> {
    await fetch('/api/v1/upstreams/remove', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ group, addr }),
    });
  },

  async getRuleContent(id: string): Promise<{ id: string; content: string }> {
    const res = await fetch(`/api/v1/rules/${id}/content`);
    return res.json();
  },

  async updateRuleContent(id: string, content: string): Promise<void> {
    await fetch(`/api/v1/rules/${id}/content`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
  },

  async getRoutingRuleContent(fileName: string): Promise<{ file_name: string; content: string }> {
    const res = await fetch(`/api/v1/routing/rules/${fileName}/content`);
    return res.json();
  },

  async updateRoutingRuleContent(fileName: string, content: string): Promise<void> {
    await fetch(`/api/v1/routing/rules/${fileName}/content`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
  },

  async getRawConfig(): Promise<{ content: string }> {
    const res = await fetch('/api/v1/config/raw');
    return res.json();
  },

  async updateRawConfig(content: string): Promise<void> {
    const res = await fetch('/api/v1/config/raw', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content }),
    });
    if (!res.ok) {
      const err = await res.text();
      throw new Error(err);
    }
  },
};
