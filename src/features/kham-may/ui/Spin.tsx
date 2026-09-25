import { Spinner } from '@fluentui/react-components';

/** Vòng quay có nhãn. Lớp `wfu-busy` để luật reduced-motion chung của v0.1 (styles.css) cũng áp vào;
 *  `km.css` có luật riêng tương đương để tab này tự đủ trước khi gộp. */
export function Spin({ label, size = 'tiny' }: { label?: string; size?: 'extra-tiny' | 'tiny' | 'small' | 'medium' }) {
  return (
    <span className="wfu-busy km-busy">
      <Spinner size={size} label={label} labelPosition="after" />
    </span>
  );
}
