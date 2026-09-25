import { Spinner } from '@fluentui/react-components';

export function Busy({ label, size = 'tiny' }: { label?: string; size?: 'tiny' | 'small' | 'medium' }) {
  return (
    <span className="wfu-busy">
      <Spinner size={size} label={label} labelPosition="after" />
    </span>
  );
}
