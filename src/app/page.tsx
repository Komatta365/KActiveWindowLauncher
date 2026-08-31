'use client';

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { createDefaultSettings, type DataType, type LauncherSettings, type LauncherSlot, SLOT_COUNT } from '@/lib/launcher';

const slotIcons: Record<DataType, string> = {
  none: '＋',
  folder: '📁',
  exe: '⚙',
  script: '📝',
  url: '🔗',
  text: 'TXT',
  image: '🖼',
  doc: '📄',
  cpl: '🧩',
  otherApp: 'APP',
};

const slotLabel = (slot: LauncherSlot) => {
  if (slot.dataType === 'none') {
    return 'Empty';
  }
  return slot.path.split(/[\\/]/).pop() || slot.dataType;
};

export default function HomePage() {
  const [settings, setSettings] = useState<LauncherSettings>(createDefaultSettings());
  const [error, setError] = useState<string | null>(null);
  const [tracked, setTracked] = useState(false);
  const [activeEdit, setActiveEdit] = useState<LauncherSlot | null>(null);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const loaded = await invoke<LauncherSettings>('settings_load');
        setSettings(loaded);
      } catch (e) {
        const message = typeof e === 'string' ? e : '設定の読み込みに失敗しました';
        setError(message);
      }
    };

    void loadSettings();

    const unlistenWindowTracked = listen<{ visible: boolean }>('launcher://window-tracked', (event) => {
      setTracked(Boolean(event.payload.visible));
    });

    const unlistenReferenceChanged = listen<{ hwnd: string | null; locked: boolean }>('launcher://reference-window-changed', () => {
      // 監視中の参照ウィンドウ状態に合わせて UI を更新する。
    });

    const unlistenTrackingError = listen<{ message: string }>('launcher://tracking-error', (event) => {
      setError(event.payload.message);
    });

    void Promise.all([unlistenWindowTracked, unlistenReferenceChanged, unlistenTrackingError]).catch(() => undefined);

    void invoke('launcher_init').catch((e) => setError(String(e)));
  }, []);

  const bars = useMemo(() => settings.slots, [settings.slots]);

  const saveSettings = async (nextSettings: LauncherSettings) => {
    setSettings(nextSettings);
    try {
      await invoke('settings_save', { settings: nextSettings });
    } catch (e) {
      setError(String(e));
    }
  };

  const updateSlot = async (slot: LauncherSlot) => {
    const next = {
      ...settings,
      slots: settings.slots.map((item) => (item.index === slot.index ? slot : item)),
    };
    await saveSettings(next);
  };

  const handleSlotClick = async (slot: LauncherSlot) => {
    if (slot.dataType === 'none') {
      setActiveEdit({ ...slot });
      return;
    }

    if (!slot.exist) {
      setActiveEdit({ ...slot });
      return;
    }

    try {
      await invoke('slot_execute', { index: slot.index });
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSaveEdit = async () => {
    if (!activeEdit) {
      return;
    }

    const dataType = await invoke<DataType>('path_detect_data_type', { path: activeEdit.path || '' });
    const nextSlot: LauncherSlot = {
      ...activeEdit,
      dataType: activeEdit.path ? dataType : 'none',
      exist: Boolean(activeEdit.path) && (await invoke<boolean>('path_exists', { path: activeEdit.path, dataType })),
    };

    await updateSlot(nextSlot);
    setActiveEdit(null);
  };

  const handleClearEdit = async () => {
    if (!activeEdit) return;
    const nextSlot: LauncherSlot = {
      ...activeEdit,
      dataType: 'none',
      path: '',
      arg: '',
      comment: '',
      exist: false,
    };
    await updateSlot(nextSlot);
    setActiveEdit(null);
  };

  return (
    <main className="page">
      <div className="launcher-shell">
        <header className="launcher-header">
          <h1 className="header-title">KActiveWindowLauncher</h1>
          <div className="header-actions">
            <span className="status-pill">
              <span className="dot" />
              {tracked ? 'Tracking' : 'Standby'}
            </span>
            <button className="icon-button primary" type="button" onClick={() => void invoke('launcher_start_tracking')}>
              Lock
            </button>
            <button className="icon-button" type="button" onClick={() => void invoke('launcher_stop_tracking')}>
              Stop
            </button>
          </div>
        </header>

        {error ? <div className="message">{error}</div> : null}

        <div className="bar">
          <button className="icon-button" type="button" aria-label="Move left">←</button>
          <button className="icon-button" type="button" aria-label="Move right">→</button>
          <button className="icon-button" type="button" aria-label="Close">×</button>
        </div>

        <div className="slot-grid">
          {bars.map((slot) => (
            <button
              key={slot.index}
              type="button"
              className={`slot ${slot.dataType === 'none' ? 'empty' : ''} ${slot.exist === false && slot.dataType !== 'none' ? 'invalid' : ''}`}
              onClick={() => void handleSlotClick(slot)}
              onContextMenu={(event) => {
                event.preventDefault();
                setActiveEdit({ ...slot });
              }}
            >
              <span className="slot-label">{slot.index + 1}</span>
              <span className="slot-content">
                <span className="slot-icon">{slotIcons[slot.dataType]}</span>
                <span className="slot-name">{slot.dataType === 'none' ? 'Empty' : slotLabel(slot)}</span>
              </span>
            </button>
          ))}
        </div>
      </div>

      {activeEdit ? (
        <div className="dialog-backdrop" onClick={() => setActiveEdit(null)}>
          <div className="dialog" onClick={(event) => event.stopPropagation()}>
            <div className="dialog-header">
              <strong>Slot {activeEdit.index + 1}</strong>
              <button type="button" className="secondary-button" onClick={() => setActiveEdit(null)}>
                Close
              </button>
            </div>
            <div className="dialog-body">
              <div className="form-row">
                <label>Path</label>
                <input
                  value={activeEdit.path}
                  onChange={(event) => setActiveEdit({ ...activeEdit, path: event.target.value })}
                  placeholder="C:\\path\\to\\target"
                />
              </div>
              <div className="form-row">
                <label>Arguments</label>
                <input
                  value={activeEdit.arg}
                  onChange={(event) => setActiveEdit({ ...activeEdit, arg: event.target.value })}
                  placeholder="Optional arguments"
                />
              </div>
              <div className="form-row">
                <label>Comment</label>
                <textarea
                  value={activeEdit.comment}
                  onChange={(event) => setActiveEdit({ ...activeEdit, comment: event.target.value })}
                  placeholder="Comment to display in tooltip"
                />
              </div>
            </div>
            <div className="dialog-footer">
              <button type="button" className="danger-button" onClick={() => void handleClearEdit()}>
                Clear
              </button>
              <div style={{ display: 'flex', gap: 8 }}>
                <button type="button" className="secondary-button" onClick={() => setActiveEdit(null)}>
                  Cancel
                </button>
                <button type="button" className="primary-button" onClick={() => void handleSaveEdit()}>
                  Save
                </button>
              </div>
            </div>
          </div>
        </div>
      ) : null}
    </main>
  );
}
