import { useState } from 'react';
import { Tab, TabList } from '@fluentui/react-components';
import type { KhamMayApi, Notify } from './api/types';
import { khamMayTauriApi } from './api/tauri';
import { DiskView } from './disk/DiskView';
import { tk } from './i18n';
import { OverviewView, type NavTarget } from './overview/OverviewView';
import { PerfView } from './perf/PerfView';
import './km.css';

type Sub = 'overview' | 'disk' | 'perf';

/**
 * Tab «Khám máy» (spec mục 2): ba mục con, mặc định Tổng quan.
 * Tổng quan luôn gắn, Ổ đĩa gắn từ lần đầu mở rồi giữ luôn — chuyển mục chỉ ẩn bằng `hidden`, nên không
 * khám/quét lại. Bộ nhớ & Hiệu năng chỉ gắn khi đang xem, vì `PerfView` lấy mẫu theo vòng đời gắn/gỡ:
 * rời mục ⇒ gỡ ⇒ `perfStop` (spec 5). `active = false` (App đang hiện tab khác) cũng coi là rời mục.
 */
export function KhamMayTab({
  api = khamMayTauriApi,
  notify,
  onGoClean,
  active = true,
}: {
  api?: KhamMayApi;
  notify: Notify;
  onGoClean?: () => void;
  active?: boolean;
}) {
  const [sub, setSub] = useState<Sub>('overview');
  const [diskVisited, setDiskVisited] = useState(false);

  function go(s: Sub) {
    if (s === 'disk') setDiskVisited(true);
    setSub(s);
  }

  function navigate(t: NavTarget) {
    if (t === 'clean') onGoClean?.();
    else go(t);
  }

  return (
    <div className="km-root">
      <TabList selectedValue={sub} onTabSelect={(_, d) => go(d.value as Sub)}>
        <Tab value="overview">{tk('km.tab.overview')}</Tab>
        <Tab value="disk">{tk('km.tab.disk')}</Tab>
        <Tab value="perf">{tk('km.tab.perf')}</Tab>
      </TabList>
      <div hidden={sub !== 'overview'}>
        <OverviewView api={api} notify={notify} onNavigate={navigate} />
      </div>
      {diskVisited && (
        <div hidden={sub !== 'disk'}>
          <DiskView api={api} notify={notify} />
        </div>
      )}
      {active && sub === 'perf' && <PerfView api={api} notify={notify} />}
    </div>
  );
}
