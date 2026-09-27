import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useEscCloseTopmost } from '../hooks/useEscClose';
import { useModalFocusTrap } from '../hooks/useModalFocusTrap';
import { useModalScrollLock } from '../hooks/useModalScrollLock';
import './CodexCliDaemonNotice.css';

/** Independent of launch progress: writing auth succeeds even if app launch fails. */
export function CodexCliDaemonNotice() {
  const { t } = useTranslation();
  const [command, setCommand] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyFailed, setCopyFailed] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const close = () => setCommand(null);
  useEscCloseTopmost(command !== null, close);
  useModalFocusTrap(root, command !== null);
  useModalScrollLock(command !== null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<string>('codex:cli-daemon-restart-required', ({ payload }) => {
      if (disposed || typeof payload !== 'string' || !payload) return;
      setCommand(payload);
      setCopied(false);
      setCopyFailed(false);
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  if (!command) return null;

  const copyCommand = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setCopyFailed(false);
    } catch {
      setCopyFailed(true);
    }
  };

  return createPortal(
    <div className="modal-overlay codex-cli-daemon-overlay">
      <div ref={root} className="modal" role="dialog" aria-modal="true"
        aria-labelledby="codex-cli-daemon-title" tabIndex={-1}>
        <div className="modal-header">
          <h2 id="codex-cli-daemon-title">{t('codex.cliDaemon.title')}</h2>
        </div>
        <div className="modal-body codex-cli-daemon-body">
          <p>{t('codex.cliDaemon.description')}</p>
          <pre><code>{command}</code></pre>
          {copyFailed && <p role="alert">{t('common.shared.export.copyFailed')}</p>}
        </div>
        <div className="modal-footer">
          <button type="button" className="btn btn-secondary" onClick={() => void copyCommand()}>
            {copied ? t('common.copied') : t('common.copy')}
          </button>
          <button type="button" className="btn btn-primary" onClick={close}>
            {t('common.close')}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
