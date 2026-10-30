/**
 * 统一时间展示工具 (完全以后端本地时间为准)
 * 后端统一按服务器本地时区 (Asia/Shanghai) 直出 RFC3339 字符串 (如 2026-10-30T15:35:12+08:00)，
 * 前端直接提取展示，完全尊崇后端时间，不做时区重算。
 */

/**
 * 提取时分秒: HH:mm:ss
 */
export function formatTime(timeStr?: string | null): string {
  if (!timeStr) return '-';
  const s = timeStr.trim();
  // 若包含 T，取 T 之后的前 8 位 HH:mm:ss
  if (s.includes('T')) {
    const afterT = s.split('T')[1];
    if (afterT && afterT.length >= 8) {
      return afterT.substring(0, 8);
    }
  }
  // 兼容普通空格分隔
  if (s.includes(' ')) {
    const afterSpace = s.split(' ')[1];
    if (afterSpace && afterSpace.length >= 8) {
      return afterSpace.substring(0, 8);
    }
  }
  const match = s.match(/(\d{2}:\d{2}:\d{2})/);
  return match ? match[1] : s;
}

/**
 * 完整日期时间: YYYY-MM-DD HH:mm:ss
 */
export function formatDateTime(timeStr?: string | null): string {
  if (!timeStr) return '-';
  const s = timeStr.trim();
  const datePart = s.match(/\d{4}-\d{2}-\d{2}/)?.[0];
  const timePart = formatTime(s);
  if (datePart && timePart && timePart !== '-') {
    return `${datePart} ${timePart}`;
  }
  return s.replace('T', ' ').substring(0, 19);
}

/**
 * 仅日期: YYYY-MM-DD
 */
export function formatDate(timeStr?: string | null): string {
  if (!timeStr) return '-';
  const s = timeStr.trim();
  const match = s.match(/(\d{4}-\d{2}-\d{2})/);
  return match ? match[1] : s;
}
