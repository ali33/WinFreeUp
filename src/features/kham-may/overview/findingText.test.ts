import { describe, expect, it } from 'vitest';
import type { Finding, FindingId, Level } from '../api/types';
import { hasKey } from '../i18n';
import { actionsFor, countProblems, FINDING_ORDER, findingSentence, levelLabel } from './findingText';

const f = (id: FindingId, level: Level, value: number | null = null, detail: string | null = null): Finding => ({ id, level, value, detail });

describe('câu giải thích khám nhanh', () => {
  it('mỗi dòng có câu cho mức Ổn và mức vấn đề', () => {
    for (const id of FINDING_ORDER) {
      expect(hasKey(`km.finding.${id}.ok`), id).toBe(true);
      expect(hasKey(`km.finding.${id}.warn`) || hasKey(`km.finding.${id}.critical`), id).toBe(true);
    }
  });

  it('số thập phân kiểu Việt Nam và tên ổ trong câu', () => {
    expect(findingSentence(f('disk_full', 'warn', 9.5))).toBe('Ổ hệ thống chỉ còn 9,5% trống — máy dễ chậm và thiếu chỗ cập nhật.');
    expect(findingSentence(f('disk_health', 'critical', null, 'WDC WD10EZEX'))).toContain('«WDC WD10EZEX»');
    expect(findingSentence(f('uptime', 'warn', 8))).toContain('8 ngày');
    expect(findingSentence(f('uptime', 'warn', 8))).toContain('Fast Startup');
  });

  it('không đo được thì ghi rõ lý do nguyên văn, không bao giờ là Ổn', () => {
    const s = findingSentence(f('cpu_hot', 'unknown', null, 'HRESULT Call failed with: 0x8004100C'));
    expect(s).toBe('Không đo được: HRESULT Call failed with: 0x8004100C');
    expect(levelLabel('unknown')).toBe('⚪ Không đo được');
  });

  it('mức nghiêm trọng không có câu riêng thì dùng câu nên xử lý', () => {
    expect(findingSentence(f('system_hdd', 'critical'))).toContain('HDD');
  });

  it('nút xử lý đúng bảng spec, chỉ khi có vấn đề', () => {
    expect(actionsFor(f('disk_full', 'critical', 3))).toEqual(['clean', 'disk']);
    expect(actionsFor(f('power_saver', 'warn'))).toEqual(['power']);
    expect(actionsFor(f('cpu_hot', 'warn', 95))).toEqual(['cooling']);
    expect(actionsFor(f('disk_full', 'ok', 40))).toEqual([]);
    expect(actionsFor(f('cpu_hot', 'unknown'))).toEqual([]);
  });

  it('đếm vấn đề', () => {
    expect(countProblems([f('disk_full', 'critical'), f('uptime', 'warn'), f('ram_pressure', 'warn'), f('cpu_hot', 'unknown')])).toEqual({ critical: 1, warn: 2 });
  });
});
