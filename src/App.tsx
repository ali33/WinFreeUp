import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { Badge, FluentProvider, Title2, webDarkTheme, webLightTheme } from '@fluentui/react-components';
import type { Api } from './api/types';
import { tauriApi } from './api/tauri';
import { dismissNotice, installGlobalHooks, pushNotice, type Notice } from './errors/errors';
import { t } from './i18n';
import { initialState, reducer } from './state/machine';
import { createStore } from './state/store';
import * as c from './state/controller';
import { NoticeBar } from './components/NoticeBar';
import { ErrorBoundary } from './components/ErrorBoundary';
import { WelcomeView } from './components/WelcomeView';
import { ScanningView } from './components/ScanningView';
import { PreviewView } from './components/PreviewView';
import { ConfirmDialog } from './components/ConfirmDialog';
import { RestorePointView } from './components/RestorePointView';
import { CleaningView } from './components/CleaningView';
import { ResultView } from './components/ResultView';

const DARK = '(prefers-color-scheme: dark)';

function usePrefersDark(): boolean {
  const [dark, setDark] = useState(() => window.matchMedia(DARK).matches);
  useEffect(() => {
    const m = window.matchMedia(DARK);
    const on = (e: MediaQueryListEvent) => setDark(e.matches);
    m.addEventListener('change', on);
    return () => m.removeEventListener('change', on);
  }, []);
  return dark;
}

export function App({ api = tauriApi }: { api?: Api }) {
  const [store] = useState(() => createStore(reducer, initialState));
  const state = useSyncExternalStore(store.subscribe, store.getState);
  const [notices, setNotices] = useState<Notice[]>([]);
  const notify = useCallback<c.Notify>((severity, message) => {
    (severity === 'error' ? console.error : console.warn)('[WinFreeUp]', message);
    setNotices((l) => pushNotice(l, severity, message));
  }, []);
  const deps = useMemo(() => c.createDeps(api, store, notify), [api, store, notify]);
  useEffect(() => installGlobalHooks(window, notify), [notify]);
  useEffect(() => {
    void c.loadInitial(deps);
  }, [deps]);
  const dark = usePrefersDark();
  const run = (p: Promise<void>) => {
    p.catch((e) => notify('error', c.friendly(e)));
  };

  return (
    <FluentProvider theme={dark ? webDarkTheme : webLightTheme} className="wfu-root">
      <main className="wfu-app">
        <header className="wfu-header">
          <Title2 as="h1">{t('app.title')}</Title2>
          {state.dryRun && (
            <Badge appearance="filled" color="warning">
              {t('app.dryRunBadge')}
            </Badge>
          )}
        </header>
        <NoticeBar notices={notices} onDismiss={(id) => setNotices((l) => dismissNotice(l, id))} />
        <ErrorBoundary onReset={() => run(c.goHome(deps))}>
          {state.phase === 'welcome' && <WelcomeView state={state} onScan={() => run(c.startScan(deps))} />}
          {state.phase === 'scanning' && <ScanningView state={state} onCancel={() => run(c.cancelScan(deps))} />}
          {(state.phase === 'preview' || state.phase === 'confirm') && (
            <PreviewView
              state={state}
              onToggle={(id) => c.toggle(deps, id)}
              onClean={() => run(c.requestClean(deps))}
              onRescan={() => run(c.startScan(deps))}
            />
          )}
          {state.phase === 'confirm' && (
            <ConfirmDialog state={state} onAccept={() => run(c.acceptConfirm(deps))} onCancel={() => c.cancelConfirm(deps)} />
          )}
          {state.phase === 'restorePoint' && (
            <RestorePointView
              state={state}
              onContinue={() => run(c.continueAfterRestoreFailure(deps))}
              onAbort={() => c.abortAfterRestoreFailure(deps)}
            />
          )}
          {state.phase === 'cleaning' && <CleaningView state={state} />}
          {state.phase === 'result' && <ResultView state={state} onOpenLog={() => c.openLog(deps)} onHome={() => run(c.goHome(deps))} />}
        </ErrorBoundary>
      </main>
    </FluentProvider>
  );
}
