import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button, MessageBar, MessageBarActions, MessageBarBody } from '@fluentui/react-components';
import { t } from '../i18n';

interface Props {
  children: ReactNode;
  onReset: () => void;
}

export class ErrorBoundary extends Component<Props, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <MessageBar intent="error" className="wfu-loi">
          <MessageBarBody>{t('errors.crashed', { message: this.state.error.message })}</MessageBarBody>
          <MessageBarActions>
            <Button
              onClick={() => {
                this.setState({ error: null });
                this.props.onReset();
              }}
            >
              {t('result.home')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      );
    }
    return this.props.children;
  }
}
