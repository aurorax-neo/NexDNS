/**
 * 判断客户端 IP 是否属于私有/内网地址段。
 * 覆盖 IPv4 私网、回环、链路本地、CGNAT，以及 IPv6 唯一本地/链路本地/回环与 IPv4 映射地址。
 */
export const isPrivateIp = (ip: string): boolean => {
  const raw = (ip || '').trim().toLowerCase().replace(/^\[|\]$/g, '');
  if (!raw) return false;

  // IPv4-mapped IPv6 (::ffff:192.168.1.1)
  const mapped = raw.match(/^::ffff:(\d{1,3}(?:\.\d{1,3}){3})$/);
  if (mapped) return isPrivateIp(mapped[1]);

  if (raw.includes(':')) {
    if (raw === '::1') return true;
    const firstGroup = parseInt(raw.split(':')[0] || '0', 16);
    if (Number.isNaN(firstGroup)) return false;
    // fc00::/7 唯一本地地址
    if ((firstGroup & 0xfe00) === 0xfc00) return true;
    // fe80::/10 链路本地地址
    if ((firstGroup & 0xffc0) === 0xfe80) return true;
    return false;
  }

  const parts = raw.split('.');
  if (parts.length !== 4) return false;
  const octets = parts.map((p) => Number(p));
  if (octets.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) return false;

  const [a, b] = octets;
  if (a === 10) return true; // 10.0.0.0/8
  if (a === 127) return true; // 127.0.0.0/8
  if (a === 0) return true; // 0.0.0.0/8
  if (a === 169 && b === 254) return true; // 169.254.0.0/16 链路本地
  if (a === 172 && b >= 16 && b <= 31) return true; // 172.16.0.0/12
  if (a === 192 && b === 168) return true; // 192.168.0.0/16
  if (a === 100 && b >= 64 && b <= 127) return true; // 100.64.0.0/10 CGNAT
  return false;
};
