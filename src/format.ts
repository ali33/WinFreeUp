const GB = 1024 ** 3;
const MB = 1024 ** 2;
const oneDecimal = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });
const integer = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 0 });

export function formatBytes(n: number): string {
  if (!(n > 0)) return '0 MB';
  if (n >= GB) return `${oneDecimal.format(n / GB)} GB`;
  if (n >= MB) return `${integer.format(n / MB)} MB`;
  return '< 1 MB';
}
