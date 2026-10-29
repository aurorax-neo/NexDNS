export interface StatusResponse {
  version: string;
  uptime_seconds: number;
  dns_port: number;
  web_port: number;
  cache_entries: number;
  adblock_rules_count: number;
  anti_pollution: boolean;
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
};
