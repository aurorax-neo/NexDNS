import React, { useState } from 'react';
import { HourlyMetric } from '../../lib/api';

export interface ActivityChartProps {
  data: HourlyMetric[];
}

export const ActivityChart: React.FC<ActivityChartProps> = ({ data }) => {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);

  if (!data || data.length === 0) {
    return (
      <div className="h-44 flex items-center justify-center text-xs text-neutral-400">
        暂无历史数据
      </div>
    );
  }

  const maxVal = Math.max(...data.map((d) => d.total), 10);
  const width = 800;
  const height = 160;
  const paddingBottom = 24;
  const paddingTop = 12;
  const chartHeight = height - paddingBottom - paddingTop;

  const points = data.map((d, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = paddingTop + chartHeight - (d.total / maxVal) * chartHeight;
    return { x, y, data: d };
  });

  const pathD = points.reduce((acc, pt, i) => {
    return i === 0 ? `M ${pt.x} ${pt.y}` : `${acc} L ${pt.x} ${pt.y}`;
  }, '');

  const areaD = `${pathD} L ${width} ${paddingTop + chartHeight} L 0 ${paddingTop + chartHeight} Z`;

  return (
    <div className="relative w-full">
      <div className="w-full overflow-hidden">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          className="w-full h-44 overflow-visible"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="queryGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#06b6d4" stopOpacity="0.35" />
              <stop offset="100%" stopColor="#06b6d4" stopOpacity="0.0" />
            </linearGradient>
          </defs>

          {/* 水平参考刻度线 */}
          {[0, 0.5, 1].map((ratio) => {
            const y = paddingTop + chartHeight * (1 - ratio);
            return (
              <line
                key={ratio}
                x1="0"
                y1={y}
                x2={width}
                y2={y}
                stroke="currentColor"
                className="text-neutral-200 dark:text-neutral-800/80"
                strokeDasharray="4 4"
                strokeWidth="1"
              />
            );
          })}

          {/* 填充面积 */}
          <path d={areaD} fill="url(#queryGrad)" />

          {/* 曲线主体 */}
          <path
            d={pathD}
            fill="none"
            stroke="#06b6d4"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />

          {/* 数据点与悬停触发热区 */}
          {points.map((pt, i) => (
            <g key={i}>
              <circle
                cx={pt.x}
                cy={pt.y}
                r={hoveredIndex === i ? 5 : 2.5}
                className="fill-cyan-500 transition-all duration-150"
              />
              <rect
                x={pt.x - width / data.length / 2}
                y={0}
                width={width / data.length}
                height={height}
                fill="transparent"
                className="cursor-pointer"
                onMouseEnter={() => setHoveredIndex(i)}
                onMouseLeave={() => setHoveredIndex(null)}
              />
            </g>
          ))}

          {/* X 轴时间刻度 (间隔显示) */}
          {points.map((pt, i) => {
            if (i % 4 !== 0 && i !== data.length - 1) return null;
            return (
              <text
                key={i}
                x={pt.x}
                y={height - 4}
                textAnchor="middle"
                className="text-[10px] font-mono fill-neutral-400 select-none"
              >
                {pt.data.hour}
              </text>
            );
          })}
        </svg>
      </div>

      {/* 悬停浮层 Tooltip */}
      {hoveredIndex !== null && data[hoveredIndex] && (
        <div
          className="absolute z-20 top-2 transform -translate-x-1/2 p-2.5 rounded-md bg-white dark:bg-surface-2 border border-neutral-300 dark:border-neutral-700 shadow-xl pointer-events-none text-xs"
          style={{
            left: `${(hoveredIndex / (data.length - 1)) * 100}%`,
          }}
        >
          <div className="font-mono text-neutral-500 dark:text-neutral-400 font-semibold mb-1">
            {data[hoveredIndex].hour}
          </div>
          <div className="flex items-center gap-2 text-neutral-800 dark:text-neutral-200">
            <span>查询总数:</span>
            <span className="font-mono font-bold tabular-nums text-cyan-600 dark:text-cyan-400">
              {data[hoveredIndex].total}
            </span>
          </div>
          <div className="flex items-center gap-2 text-neutral-800 dark:text-neutral-200">
            <span>已拦截:</span>
            <span className="font-mono font-bold tabular-nums text-rose-500">
              {data[hoveredIndex].blocked}
            </span>
          </div>
          {data[hoveredIndex].total > 0 && (
            <div className="flex items-center gap-2 text-neutral-500 text-[11px] mt-0.5">
              <span>均时延:</span>
              <span className="font-mono tabular-nums">
                {data[hoveredIndex].avg_duration_ms.toFixed(1)} ms
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
