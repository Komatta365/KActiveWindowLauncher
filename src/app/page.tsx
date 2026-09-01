'use client';

import { type DragEvent, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { createDefaultSettings, type DataType, type LauncherSettings, type LauncherSlot } from '@/lib/launcher';

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

const SLOT_BUTTON_PITCH = 36;
const SYSTEM_BAR_WIDTH = 180;

export default function HomePage() {
  const [settings, setSettings] = useState<LauncherSettings>(createDefaultSettings());
  const [slotStartIndex, setSlotStartIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [trackedWidth, setTrackedWidth] = useState(960);
  const [locked, setLocked] = useState(false);
  const [slotRowWidth, setSlotRowWidth] = useState(0);
  const slotRowRef = useRef<HTMLDivElement>(null);

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

    const unlistenWindowTracked = listen<{ visible: boolean; width: number }>('launcher://window-tracked', (event) => {
      if (event.payload.visible) {
        setTrackedWidth(event.payload.width);
      }
    });

    const unlistenReferenceChanged = listen<{ hwnd: string | null; locked: boolean }>(
      'launcher://reference-window-changed',
      (event) => {
        setLocked(Boolean(event.payload.locked));
      },
    );

    const unlistenTrackingError = listen<{ message: string }>('launcher://tracking-error', (event) => {
      setError(event.payload.message);
    });

    const unlistenSettingsUpdated = listen('launcher://settings-updated', async () => {
      try {
        const loaded = await invoke<LauncherSettings>('settings_load');
        setSettings(loaded);
      } catch (e) {
        setError(String(e));
      }
    });

    void Promise.all([unlistenWindowTracked, unlistenReferenceChanged, unlistenTrackingError, unlistenSettingsUpdated]).catch(() => undefined);

    void invoke('launcher_init').catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    const row = slotRowRef.current;
    if (!row) {
      return;
    }

    const resizeObserver = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      setSlotRowWidth(width);
    });
    resizeObserver.observe(row);
    setSlotRowWidth(row.clientWidth);

    return () => {
      resizeObserver.disconnect();
    };
  }, []);

  const visibleSlotCount = useMemo(
    () => {
      const availableWidth = slotRowWidth > 0 ? slotRowWidth : Math.max(trackedWidth - SYSTEM_BAR_WIDTH, 0);
      return Math.max(1, Math.min(settings.slots.length, Math.floor(availableWidth / SLOT_BUTTON_PITCH)));
    },
    [settings.slots.length, slotRowWidth, trackedWidth],
  );

  const visibleSlots = useMemo(
    () => settings.slots.slice(slotStartIndex, slotStartIndex + visibleSlotCount),
    [settings.slots, slotStartIndex, visibleSlotCount],
  );

  const updateSlotByPath = async (slotIndex: number, path: string) => {
    const dataType = (await invoke<DataType>('path_detect_data_type', { path })) || 'none';
    const nextSlot: LauncherSlot = {
      index: slotIndex,
      dataType,
      path,
      arg: '',
      comment: '',
      exist: Boolean(path) && (await invoke<boolean>('path_exists', { path, dataType })),
    };
    const nextSettings = {
      ...settings,
      slots: settings.slots.map((item) => (item.index === slotIndex ? nextSlot : item)),
    };
    setSettings(nextSettings);
    await invoke('settings_save', { settings: nextSettings });
  };

  const openSlotEditor = async (slotIndex: number) => {
    try {
      await invoke('launcher_open_slot_editor', { index: slotIndex });
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSlotClick = async (slot: LauncherSlot) => {
    if (slot.dataType === 'none' || !slot.exist) {
      await openSlotEditor(slot.index);
      return;
    }

    try {
      await invoke('slot_execute', { index: slot.index });
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDragDrop = async (slot: LauncherSlot, event: DragEvent<HTMLButtonElement>) => {
    event.preventDefault();
    const text = event.dataTransfer.getData('text/uri-list') || event.dataTransfer.getData('text/plain');
    const fileList = Array.from(event.dataTransfer.files);
    const firstFile = fileList[0] as (File & { path?: string }) | undefined;
    const sourcePath = firstFile?.path || text.trim();

    if (!sourcePath) {
      return;
    }

    const withoutProtocol = sourcePath.replace(/^file:\/\//i, '');
    const normalizedPath = withoutProtocol.replace(/^file:\//i, '');

    if (slot.dataType === 'none') {
      await updateSlotByPath(slot.index, normalizedPath);
      return;
    }

    const droppedArg = /\s/.test(normalizedPath) ? `"${normalizedPath}"` : normalizedPath;
    await invoke('slot_execute', { index: slot.index, droppedArg });
  };

  const hideToTray = async () => {
    try {
      await invoke('launcher_hide_to_tray');
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleLock = async () => {
    try {
      if (locked) {
        await invoke('launcher_stop_tracking');
      } else {
        await invoke('launcher_start_tracking');
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const scrollSlots = (direction: number) => {
    const nextIndex = slotStartIndex + direction * visibleSlotCount;
    setSlotStartIndex(Math.min(Math.max(nextIndex, 0), Math.max(settings.slots.length - visibleSlotCount, 0)));
  };

  useEffect(() => {
    setSlotStartIndex((current) => Math.min(current, Math.max(settings.slots.length - visibleSlotCount, 0)));
  }, [settings.slots.length, visibleSlotCount]);

  return (
    <main className="page" data-theme={settings.theme}>
      <div className="launcher-shell">
        <div className="launcher-bar">
          <button
            className={`system-button system-button-icon system-button-lock ${locked ? 'active' : ''}`}
            type="button"
            onClick={() => void toggleLock()}
            aria-label={locked ? 'Unlock' : 'Lock'}
            title={locked ? 'Unlock' : 'Lock'}
          >
            <span className="system-button-image" aria-hidden="true" />
          </button>
          <button
            className="system-button system-button-icon system-button-left"
            type="button"
            onClick={() => scrollSlots(-1)}
            aria-label="Move left"
            title="Move left"
            disabled={slotStartIndex <= 0}
          >
            <span className="system-button-image" aria-hidden="true" />
          </button>
          <button
            className="system-button system-button-icon system-button-right"
            type="button"
            onClick={() => scrollSlots(1)}
            aria-label="Move right"
            title="Move right"
            disabled={slotStartIndex + visibleSlotCount >= settings.slots.length}
          >
            <span className="system-button-image" aria-hidden="true" />
          </button>

          <div className="slot-row" ref={slotRowRef}>
            {visibleSlots.map((slot) => (
              <button
                key={slot.index}
                type="button"
                className={`slot ${slot.dataType === 'none' ? 'empty' : ''} ${slot.exist === false && slot.dataType !== 'none' ? 'invalid' : ''}`}
                onClick={() => void handleSlotClick(slot)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  void openSlotEditor(slot.index);
                }}
                onDragOver={(event) => event.preventDefault()}
                onDrop={(event) => void handleDragDrop(slot, event)}
              >
                <span className="slot-label">{slot.index + 1}</span>
                <span className="slot-icon">{slotIcons[slot.dataType]}</span>
                <span className="slot-name">{slot.dataType === 'none' ? 'Empty' : slotLabel(slot)}</span>
              </button>
            ))}
          </div>
          <button
            className="system-button system-button-icon system-button-close"
            type="button"
            onClick={() => void hideToTray()}
            aria-label="Close"
            title="Close"
          >
            <span className="system-button-image" aria-hidden="true" />
          </button>
        </div>

        {error ? <div className="message">{error}</div> : null}
      </div>
    </main>
  );
}
