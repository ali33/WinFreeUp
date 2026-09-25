import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import { app, GB, sample } from '../testing/fakeApi';
import { LineChart } from './LineChart';
import { netTotal, niceMax, pushSample, segments, sortApps, WINDOW_MS } from './perfModel';

describe('chuỗi mẫu', () => {
  it('giữ đúng 5 phút gần nhất, bỏ trùng giờ', () => {
    let l = [sample(1000)];
    l = pushSample(l, sample(1000));
    expect(l).toHaveLength(1);
    for (let i = 1; i <= 400; i++) l = pushSample(l, sample(1000 + i * 1000));
    expect(l[l.length - 1].t_ms - l[0].t_ms).toBe(WINDOW_MS);
  });

  it('đường bị ngắt ở chỗ thiếu số liệu và ở khoảng dừng lấy mẫu', () => {
    const s = [sample(1000), sample(2000), { ...sample(3000), disk_active: null }, sample(4000), sample(60_000)];
    const segs = segments(s, (x) => x.disk_active);
    expect(segs.map((g) => g.map((p) => p.t))).toEqual([[1000, 2000], [4000], [60_000]]);
  });

  it('trục mạng tự co theo lũy thừa 2, tối thiểu 64 KB/s', () => {
    expect(niceMax([])).toBe(64 * 1024);
    expect(niceMax([3 * 1024 ** 2])).toBe(4 * 1024 ** 2);
  });
});

describe('bảng ứng dụng', () => {
  const apps = [
    app('a', 'Zalo', GB, { cpu: 5, disk_bps: 10, net_up_bps: 1, net_down_bps: 1 }),
    app('b', 'Chrome', 3 * GB, { cpu: 1, disk_bps: 50, net_up_bps: 100, net_down_bps: 900 }),
    app('c', 'Ảnh', 2 * GB, { cpu: 9, disk_bps: 0, net_up_bps: null, net_down_bps: null }),
  ];
  it('sắp theo mọi cột, hai chiều', () => {
    expect(sortApps(apps, 'ram', true).map((a) => a.name)).toEqual(['Chrome', 'Ảnh', 'Zalo']);
    expect(sortApps(apps, 'cpu', true).map((a) => a.name)).toEqual(['Ảnh', 'Zalo', 'Chrome']);
    expect(sortApps(apps, 'disk', false).map((a) => a.name)).toEqual(['Ảnh', 'Zalo', 'Chrome']);
    expect(sortApps(apps, 'net', true).map((a) => a.name)).toEqual(['Chrome', 'Zalo', 'Ảnh']);
    expect(sortApps(apps, 'name', false).map((a) => a.name)).toEqual(['Ảnh', 'Chrome', 'Zalo']);
  });

  it('mạng không theo dõi được thì là null, không phải 0', () => {
    expect(netTotal(apps[2])).toBeNull();
    expect(netTotal(apps[1])).toBe(1000);
  });
});

describe('LineChart', () => {
  it('vẽ một polyline mỗi đoạn và dải đỏ cho cơn giật trong cửa sổ', () => {
    const now = 400_000;
    const { container } = render(
      <LineChart
        title="CPU"
        valueText="20%"
        max={100}
        now={now}
        lines={[{ segments: [[{ t: 100_000, v: 50 }, { t: 400_000, v: 100 }], [{ t: 200_000, v: 0 }]] }]}
        bands={[
          { start_ms: 250_000, end_ms: 280_000 },
          { start_ms: 10_000, end_ms: 20_000 },
        ]}
      />,
    );
    const lines = container.querySelectorAll('polyline');
    expect(lines).toHaveLength(2);
    expect(lines[0].getAttribute('points')).toBe('0.0,50.0 300.0,0.0');
    const bands = container.querySelectorAll('[data-band]');
    expect(bands).toHaveLength(1);
    expect(bands[0].getAttribute('x')).toBe('150');
    expect(bands[0].getAttribute('width')).toBe('30');
    expect(container.querySelector('svg')?.getAttribute('aria-label')).toBe('CPU: 20%');
  });
});
