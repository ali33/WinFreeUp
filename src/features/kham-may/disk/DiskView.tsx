import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  ProgressBar,
  Select,
} from '@fluentui/react-components';
import type { KhamMayApi, NodeView, Notify, ScanPlan, ScanStatus, VolumeInfo } from '../api/types';
import { formatBytes, formatCount, formatDate, formatSeconds, friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { applyDelete, collapse, emptyTree, isHiberfil, reopen, share, startTree, visibleRows, withPage, type SortKey, type TreeModel } from './treeModel';

type Phase = 'idle' | 'scanning' | 'cancelling' | 'done';

export function walkReasonText(plan: ScanPlan): string | null {
  if (plan.mode === 'mft') return null;
  const r = plan.reason;
  const reason =
    r.code === 'not_ntfs'
      ? tk('km.disk.reason.not_ntfs', { fs: r.fs })
      : r.code === 'not_local'
        ? tk('km.disk.reason.not_local')
        : tk('km.disk.reason.mft_failed', { message: r.message });
  return tk('km.disk.slowMode', { reason });
}

/** Lõi trả đúng chuỗi `cancelled` khi người dùng tự hủy quét. So thẳng mã gốc, KHÔNG qua `friendly`
 *  (câu hiển thị có thể đổi, mã thì không). */
function isCancelled(e: unknown): boolean {
  return e === 'cancelled' || (e instanceof Error && e.message === 'cancelled');
}

/** Lõi từ chối xóa vì lý do người dùng tự xử lý được trong Explorer (OneDrive, ổ không có Thùng rác…):
 *  hiện câu dễ hiểu kèm nút «Mở trong Explorer» thay vì chỉ báo lỗi. */
const SELF_DELETE_CODES = new Set(['onedrive', 'no_recycle_bin', 'recycle_disabled', 'redirected_path']);

function pickDefault(vols: VolumeInfo[]): string {
  return (vols.find((v) => v.root.toUpperCase() === 'C:\\') ?? vols[0])?.root ?? '';
}

export function DiskView({ api, notify }: { api: KhamMayApi; notify: Notify }) {
  const [volumes, setVolumes] = useState<VolumeInfo[] | null>(null);
  const [selected, setSelected] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [progress, setProgress] = useState<ScanStatus | null>(null);
  const [tree, setTree] = useState<TreeModel>(emptyTree);
  const [elapsed, setElapsed] = useState<number | null>(null);
  const [loadingNode, setLoadingNode] = useState<number | null>(null);
  const [sort, setSort] = useState<SortKey>('size');
  const [confirm, setConfirm] = useState<NodeView | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [status, setStatus] = useState('');
  const [guide, setGuide] = useState(false);
  const [selfDelete, setSelfDelete] = useState<{ node: NodeView; message: string } | null>(null);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    api
      .diskVolumes()
      .then((v) => {
        if (!alive.current) return;
        setVolumes(v);
        setSelected(pickDefault(v));
      })
      .catch((e) => {
        if (!alive.current) return;
        setVolumes([]);
        notify('error', tk('km.disk.volumesFailed', { message: friendly(e) }));
      });
    return () => {
      alive.current = false;
    };
  }, [api, notify]);

  const rows = useMemo(() => visibleRows(tree, sort), [tree, sort]);

  async function loadChildren(id: number): Promise<void> {
    setLoadingNode(id);
    try {
      const page = await api.treeChildren(id);
      // Cập nhật theo hàm: không đè lên thay đổi xảy ra trong lúc chờ lõi.
      if (alive.current) setTree((t) => withPage(t, page));
    } catch (e) {
      if (alive.current) notify('warning', tk('km.disk.loadFailed', { message: friendly(e) }));
    } finally {
      if (alive.current) setLoadingNode(null);
    }
  }

  async function scan() {
    setPhase('scanning');
    setProgress(null);
    setStatus('');
    setElapsed(null);
    setSelfDelete(null);
    try {
      const sum = await api.diskScan(selected, (s) => alive.current && setProgress(s));
      if (!alive.current) return;
      const slow = walkReasonText(sum.plan);
      if (slow) notify('warning', slow);
      // Tải tầng đầu TRƯỚC, rồi thay cả cây một lần: tới lúc đó cây cũ vẫn mờ dưới lớp phủ.
      // Tầng đầu hỏng thì vẫn thay gốc (cây cũ đã hết hạn ở lõi), báo hổ phách, có nút Thử lại.
      let next = startTree(sum.root);
      try {
        next = withPage(next, await api.treeChildren(sum.root.id));
      } catch (e) {
        if (alive.current) notify('warning', tk('km.disk.loadFailed', { message: friendly(e) }));
      }
      if (!alive.current) return;
      setTree(next);
      setElapsed(sum.elapsed_ms);
      setPhase('done');
    } catch (e) {
      if (!alive.current) return;
      setPhase(tree.root ? 'done' : 'idle');
      if (!isCancelled(e)) notify('error', tk('km.disk.scanFailed', { message: friendly(e) }));
    }
  }

  async function cancel() {
    setPhase('cancelling');
    try {
      await api.diskScanCancel();
    } catch (e) {
      // Hủy không được thì lượt quét vẫn chạy: trả nút Hủy lại để bấm lần nữa.
      if (!alive.current) return;
      setPhase((p) => (p === 'cancelling' ? 'scanning' : p));
      notify('warning', tk('km.disk.cancelFailed', { message: friendly(e) }));
    }
  }

  function toggle(n: NodeView, expanded: boolean) {
    if (expanded) {
      setTree(collapse(tree, n.id));
      return;
    }
    const again = reopen(tree, n.id);
    if (again) setTree(again);
    else void loadChildren(n.id);
  }

  async function reveal(n: NodeView) {
    try {
      await api.diskReveal(n.id);
    } catch (e) {
      if (alive.current) notify('warning', tk('km.disk.revealFailed', { message: friendly(e) }));
    }
  }

  async function copy(n: NodeView) {
    try {
      await api.copyText(n.path);
      if (alive.current) setStatus(tk('km.disk.copied'));
    } catch (e) {
      if (alive.current) notify('warning', tk('km.disk.copyFailed', { message: friendly(e) }));
    }
  }

  async function doDelete() {
    const n = confirm;
    if (!n) return;
    setDeleting(true);
    setSelfDelete(null);
    try {
      const r = await api.diskDelete(n.id);
      if (!alive.current) return;
      setTree((t) => applyDelete(t, n.id, r.removed));
      setStatus(tk('km.disk.deleted', { name: n.name, size: formatBytes(r.removed.bytes) }));
      if (r.log_error) notify('warning', tk('km.err.logWrite', { message: r.log_error }));
    } catch (e) {
      if (!alive.current) return;
      const message = tk('km.disk.deleteFailed', { name: n.name, message: friendly(e) });
      if (typeof e === 'string' && SELF_DELETE_CODES.has(e)) setSelfDelete({ node: n, message });
      else notify('error', message);
    } finally {
      if (alive.current) {
        setDeleting(false);
        setConfirm(null);
      }
    }
  }

  const busyScan = phase === 'scanning' || phase === 'cancelling';

  return (
    <section className="km-root" aria-label={tk('km.tab.disk')}>
      <div className="km-toolbar">
        <Field label={tk('km.disk.volume')}>
          {volumes === null ? (
            <Spin />
          ) : (
            <Select value={selected} disabled={busyScan} onChange={(_, d) => setSelected(d.value)}>
              {volumes.map((v) => (
                <option key={v.root} value={v.root}>
                  {tk('km.disk.volumeOption', { root: v.root, label: v.label, free: formatBytes(v.free), total: formatBytes(v.total) })}
                </option>
              ))}
            </Select>
          )}
        </Field>
        {busyScan ? (
          <Button onClick={() => void cancel()} disabled={phase === 'cancelling'}>
            {phase === 'cancelling' ? tk('km.disk.cancelling') : tk('km.disk.cancel')}
          </Button>
        ) : (
          <Button appearance="primary" disabled={!selected || deleting || loadingNode !== null} onClick={() => void scan()}>
            {tree.root ? tk('km.disk.rescan') : tk('km.disk.scan')}
          </Button>
        )}
        {tree.root && !busyScan && (
          <Field label={tk('km.disk.sort')}>
            <Select value={sort} onChange={(_, d) => setSort(d.value as SortKey)}>
              <option value="size">{tk('km.disk.sort.size')}</option>
              <option value="name">{tk('km.disk.sort.name')}</option>
              <option value="modified">{tk('km.disk.sort.modified')}</option>
            </Select>
          </Field>
        )}
      </div>

      {busyScan && (
        <div className="km-root" aria-live="polite">
          <Spin label={tk('km.disk.scanning', { root: selected })} />
          <ProgressBar value={progress?.percent != null ? progress.percent / 100 : undefined} />
          {progress && (
            <span className="km-muted">
              {tk('km.disk.progress', { files: formatCount(progress.files), size: formatBytes(progress.bytes) })} · {progress.current}
            </span>
          )}
        </div>
      )}

      {!busyScan && !tree.root && <p className="km-muted">{tk('km.disk.hint')}</p>}
      {!busyScan && elapsed !== null && <p className="km-muted">{tk('km.disk.done', { seconds: formatSeconds(elapsed) })}</p>}
      <p className="km-muted" aria-live="polite">
        {status}
      </p>

      {selfDelete && (
        <MessageBar intent="warning" role="alert">
          <MessageBarBody>{selfDelete.message}</MessageBarBody>
          <MessageBarActions>
            <Button size="small" disabled={busyScan} onClick={() => void reveal(selfDelete.node)}>
              {tk('km.disk.reveal')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      )}

      {/* Quét lại hoặc đang mở một tầng: giữ cây cũ mờ dưới lớp phủ (lớp phủ chặn bấm), không xoá trắng. */}
      {tree.root && (
        <div className="km-overlay-host" aria-busy={busyScan || loadingNode !== null}>
          {(busyScan || loadingNode !== null) && (
            <div className="km-overlay">
              <Spin size="small" />
            </div>
          )}
          <table className="km-tree">
            <thead>
              <tr>
                <th>{tk('km.disk.col.name')}</th>
                <th className="km-num">{tk('km.disk.col.size')}</th>
                <th>{tk('km.disk.col.share')}</th>
                <th>{tk('km.disk.col.modified')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>
                  <strong>{tree.root.name}</strong> 🔒
                </td>
                <td className="km-num">{formatBytes(tree.root.bytes)}</td>
                <td />
                <td />
                <td />
              </tr>
              {!tree.pages[tree.root.id] && loadingNode === null && !busyScan && (
                <tr>
                  <td colSpan={5}>
                    <Button size="small" onClick={() => tree.root && void loadChildren(tree.root.id)}>
                      {tk('km.common.retry')}
                    </Button>
                  </td>
                </tr>
              )}
              {rows.map((r) =>
                r.kind === 'rest' ? (
                  <tr key={`rest-${r.parentId}`}>
                    <td className="km-muted" style={{ paddingLeft: 24 + r.depth * 18 }}>
                      {tk('km.disk.rest', { count: formatCount(r.rest.count), size: formatBytes(r.rest.bytes) })}
                    </td>
                    <td className="km-num km-muted">{formatBytes(r.rest.bytes)}</td>
                    <td />
                    <td />
                    <td />
                  </tr>
                ) : (
                  <tr key={r.node.id}>
                    <td style={{ paddingLeft: 6 + r.depth * 18 }}>
                      <div className="km-tree-name">
                        {r.node.is_dir && r.node.has_children && !r.node.unreadable ? (
                          <Button
                            size="small"
                            appearance="transparent"
                            aria-label={tk(r.expanded ? 'km.disk.collapse' : 'km.disk.expand', { name: r.node.name })}
                            disabled={loadingNode !== null || busyScan}
                            onClick={() => toggle(r.node, r.expanded)}
                          >
                            {r.expanded ? '▾' : '▸'}
                          </Button>
                        ) : (
                          <span style={{ width: 24, display: 'inline-block' }} />
                        )}
                        <span className="km-tree-label" title={r.node.path}>
                          {r.node.name}
                        </span>
                        {r.node.protected && <span title={tk('km.disk.protected')}>🔒</span>}
                        {r.node.unreadable && <span className="km-muted">{tk('km.disk.unreadable')}</span>}
                        {r.node.is_link && <span className="km-muted" title={tk('km.disk.link')}>↪</span>}
                        {loadingNode === r.node.id && <Spin size="extra-tiny" />}
                      </div>
                      {isHiberfil(r.node) && (
                        <div className="km-muted">
                          {tk('km.disk.hiberfil', { size: formatBytes(r.node.bytes) })}{' '}
                          <Button size="small" appearance="transparent" onClick={() => setGuide(true)}>
                            {tk('km.disk.hiberfilGuide')}
                          </Button>
                        </div>
                      )}
                    </td>
                    <td className="km-num">{formatBytes(r.node.bytes)}</td>
                    <td className="km-share">
                      <div className="km-share-bar" role="img" aria-label={`${Math.round(share(r.node.bytes, r.parentBytes))}%`}>
                        <div className="km-share-fill" style={{ width: `${share(r.node.bytes, r.parentBytes)}%` }} />
                      </div>
                    </td>
                    <td className="km-muted">{formatDate(r.node.modified)}</td>
                    <td>
                      <div className="km-row-actions">
                        <Button size="small" appearance="subtle" disabled={busyScan} onClick={() => void reveal(r.node)}>
                          {tk('km.disk.reveal')}
                        </Button>
                        <Button size="small" appearance="subtle" disabled={busyScan} onClick={() => void copy(r.node)}>
                          {tk('km.disk.copy')}
                        </Button>
                        <Button
                          size="small"
                          appearance="subtle"
                          disabled={r.node.protected || deleting || busyScan || loadingNode !== null}
                          title={r.node.protected ? tk('km.disk.protected') : undefined}
                          onClick={() => setConfirm(r.node)}
                        >
                          {tk('km.disk.delete')}
                        </Button>
                      </div>
                    </td>
                  </tr>
                ),
              )}
            </tbody>
          </table>
        </div>
      )}

      <Dialog open={confirm !== null} onOpenChange={(_, d) => !d.open && !deleting && setConfirm(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{tk('km.disk.deleteTitle')}</DialogTitle>
            <DialogContent>
              {confirm &&
                tk('km.disk.deleteBody', { name: confirm.name, size: formatBytes(confirm.bytes), files: formatCount(confirm.files) })}
            </DialogContent>
            <DialogActions>
              {deleting ? (
                <Spin label={tk('km.disk.deleting')} />
              ) : (
                <>
                  <Button appearance="primary" onClick={() => void doDelete()}>
                    {tk('km.disk.deleteConfirm')}
                  </Button>
                  <Button onClick={() => setConfirm(null)}>{tk('km.disk.deleteCancel')}</Button>
                </>
              )}
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog open={guide} onOpenChange={(_, d) => setGuide(d.open)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{tk('km.disk.hiberfilTitle')}</DialogTitle>
            <DialogContent>{tk('km.disk.hiberfilBody')}</DialogContent>
            <DialogActions>
              <Button onClick={() => setGuide(false)}>{tk('km.guide.close')}</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
