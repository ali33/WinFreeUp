export type Severity = 'error' | 'warning';

export interface Notice {
  id: number;
  severity: Severity;
  message: string;
  count: number;
}

export const MAX_NOTICES = 5;

let nextId = 1;

export function pushNotice(list: Notice[], severity: Severity, message: string): Notice[] {
  const i = list.findIndex((n) => n.severity === severity && n.message === message);
  if (i >= 0) return list.map((n, j) => (j === i ? { ...n, count: n.count + 1 } : n));
  let next = [...list, { id: nextId++, severity, message, count: 1 }];
  while (next.length > MAX_NOTICES) {
    const w = next.findIndex((n) => n.severity === 'warning');
    next = next.filter((_, j) => j !== (w >= 0 ? w : 0));
  }
  return next;
}

export function dismissNotice(list: Notice[], id: number): Notice[] {
  return list.filter((n) => n.id !== id);
}

export function messageOf(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export function describeErrorEvent(ev: { message?: string; filename?: string; lineno?: number; error?: unknown }): string {
  const msg = ev.message || messageOf(ev.error);
  const file = ev.filename ? ev.filename.split('/').pop() : '';
  return file ? `${msg} (${file}:${ev.lineno ?? 0})` : msg;
}

/** Lỗi ngoài luồng lên màn dạng hổ phách (màn vẫn dùng được); vẫn ghi console đủ ngăn xếp. */
export function installGlobalHooks(target: Window, report: (severity: Severity, message: string) => void): () => void {
  const onError = (ev: ErrorEvent) => {
    console.error(ev.error ?? ev.message);
    report('warning', describeErrorEvent(ev));
  };
  const onRejection = (ev: Event) => {
    const reason = (ev as PromiseRejectionEvent).reason;
    console.error(reason);
    report('warning', messageOf(reason));
  };
  target.addEventListener('error', onError);
  target.addEventListener('unhandledrejection', onRejection);
  return () => {
    target.removeEventListener('error', onError);
    target.removeEventListener('unhandledrejection', onRejection);
  };
}
