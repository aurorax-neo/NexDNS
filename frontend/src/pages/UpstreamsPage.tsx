import React, { useEffect, useState } from 'react';
import {
  Server,
  Zap,
  CheckCircle2,
  XCircle,
  Play,
  Globe,
  Shield,
  Radio,
} from 'lucide-react';
import { api, UpstreamTestItem, UpstreamsResponse } from '../lib/api';

export const UpstreamsPage: React.FC = () => {
  const [upstreams, setUpstreams] = useState<UpstreamsResponse | null>(null);
  const [testResults, setTestResults] = useState<UpstreamTestItem[]>([]);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    api.getUpstreams().then(setUpstreams).catch(console.error);
  }, []);

  const handleBenchmark = async () => {
    setTesting(true);
    try {
      const res = await api.testUpstreams();
      setTestResults(res);
    } catch (e) {
      console.error('Failed to test upstreams', e);
    } finally {
      setTesting(false);
    }
  };

  const getLatencyScore = (addr: string) => {
    const match = testResults.find((r) => r.addr === addr);
    if (!match) return null;
    return match;
  };

  const renderGroup = (title: string, groupKey: 'mainland' | 'gfw' | 'overseas', icon: React.ReactNode, desc: string) => {
    const servers = upstreams ? upstreams[groupKey] : [];
    return (
      <div className="p-5 rounded-lg bg-surface-1 border border-neutral-800 space-y-4">
        <div className="flex items-center justify-between pb-3 border-b border-neutral-800">
          <div className="flex items-center gap-2.5">
            {icon}
            <div>
              <h4 className="text-sm font-semibold text-neutral-100">{title}</h4>
              <p className="text-xs text-neutral-400 mt-0.5">{desc}</p>
            </div>
          </div>
          <span className="text-xs font-mono text-neutral-400 bg-surface-2 px-2 py-0.5 rounded border border-neutral-800">
            {servers.length} 个节点
          </span>
        </div>

        <div className="space-y-2.5">
          {servers.map((addr) => {
            const score = getLatencyScore(addr);
            const isDoh = addr.startsWith('https://');
            const isTcp = addr.startsWith('tcp://');
            const proto = isDoh ? 'DoH' : isTcp ? 'TCP' : 'UDP';

            return (
              <div
                key={addr}
                className="flex items-center justify-between p-3 rounded-md bg-surface-2 border border-neutral-800/80 hover:border-neutral-700 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <span
                    className={`px-1.5 py-0.5 text-[10px] font-mono font-bold rounded ${
                      isDoh
                        ? 'bg-purple-950/60 text-purple-400 border border-purple-800/40'
                        : isTcp
                        ? 'bg-blue-950/60 text-blue-400 border border-blue-800/40'
                        : 'bg-emerald-950/60 text-emerald-400 border border-emerald-800/40'
                    }`}
                  >
                    {proto}
                  </span>
                  <span className="font-mono text-xs text-neutral-200">{addr}</span>
                </div>

                <div className="flex items-center gap-2">
                  {score && (
                    <div className="flex items-center gap-1.5 font-mono text-xs tabular-nums">
                      {score.ok ? (
                        <>
                          <span
                            className={`font-semibold ${
                              score.latency_ms! < 50
                                ? 'text-emerald-400'
                                : score.latency_ms! < 150
                                ? 'text-amber-400'
                                : 'text-neutral-400'
                            }`}
                          >
                            {score.latency_ms} ms
                          </span>
                          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 align-icon" />
                        </>
                      ) : (
                        <>
                          <span className="text-rose-400">超时</span>
                          <XCircle className="w-3.5 h-3.5 text-rose-400 align-icon" />
                        </>
                      )}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      {/* 头部控制栏 */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-5 rounded-lg bg-surface-1 border border-neutral-800">
        <div>
          <h3 className="text-base font-semibold text-neutral-100 flex items-center gap-2">
            <Server className="w-4 h-4 text-cyan-400 align-icon" />
            SmartDNS 多协议上游组与测速优选
          </h3>
          <p className="text-xs text-neutral-400 mt-1 max-w-2xl leading-relaxed">
            分流引擎调度至不同上游组并发解析。支持 UDP 53、TCP 53 以及加密 DNS over HTTPS (DoH)。内置 TCP 80/443 建连测速引擎。
          </p>
        </div>
        <div>
          <button
            type="button"
            onClick={handleBenchmark}
            disabled={testing}
            className="inline-flex items-center gap-2 min-h-[38px] px-4 text-xs font-medium text-white bg-cyan-600 hover:bg-cyan-500 rounded-md transition-colors shadow-sm disabled:opacity-50"
          >
            <Play className="w-3.5 h-3.5 align-icon" />
            {testing ? '正在并发测速中...' : '立即测试全组上游延迟'}
          </button>
        </div>
      </div>

      {/* 三大上游组卡片 */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {renderGroup(
          '大陆直连组 (mainland)',
          'mainland',
          <Zap className="w-4 h-4 text-cyan-400 align-icon" />,
          '国内直连解析，支持阿里 DNS、DNSPub 等低时延节点'
        )}
        {renderGroup(
          'GFW / 境外组 (gfw)',
          'gfw',
          <Shield className="w-4 h-4 text-rose-400 align-icon" />,
          '代理加速与抗污染回退组，走纯净加密 DoH'
        )}
        {renderGroup(
          '海外备用组 (overseas)',
          'overseas',
          <Globe className="w-4 h-4 text-emerald-400 align-icon" />,
          'Cloudflare / Quad9 全球冗余应急解析'
        )}
      </div>
    </div>
  );
};
