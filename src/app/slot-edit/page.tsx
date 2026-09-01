'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { createDefaultSettings, createEmptySlot, type DataType, type LauncherSettings, type LauncherSlot } from '@/lib/launcher';

export const dynamic = 'force-dynamic';

export default function SlotEditPage() {
  const [slotIndex, setSlotIndex] = useState(0);

  const [settings, setSettings] = useState<LauncherSettings>(createDefaultSettings());
  const [slot, setSlot] = useState<LauncherSlot>(createEmptySlot(0));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    const match = currentWindow.label.match(/^slot-editor-(\d+)$/);
    const index = match ? Number(match[1]) : 0;
    setSlotIndex(index);
    setSlot(createEmptySlot(index));
  }, []);

  useEffect(() => {
    const load = async () => {
      try {
        const loaded = await invoke<LauncherSettings>('settings_load');
        setSettings(loaded);
        const target = loaded.slots[slotIndex];
        if (!target) {
          setError('スロット番号が範囲外です');
          return;
        }
        setSlot({ ...target });
      } catch (e) {
        setError(String(e));
      }
    };
    void load();
  }, [slotIndex]);

  const closeWindow = async () => {
    await getCurrentWindow().close();
  };

  const saveSlot = async (nextSlot: LauncherSlot) => {
    const nextSettings = {
      ...settings,
      slots: settings.slots.map((current) => (current.index === nextSlot.index ? nextSlot : current)),
    };
    await invoke('settings_save', { settings: nextSettings });
    await closeWindow();
  };

  const detectDataType = async (path: string) => {
    if (!path) {
      return 'none' as DataType;
    }
    return (await invoke<DataType>('path_detect_data_type', { path })) || 'none';
  };

  const pathExists = async (path: string, dataType: DataType) => {
    if (!path || dataType === 'none') {
      return false;
    }
    return invoke<boolean>('path_exists', { path, dataType });
  };

  const handleClear = async () => {
    try {
      await saveSlot(createEmptySlot(slotIndex));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSave = async () => {
    try {
      const dataType = await detectDataType(slot.path.trim());
      const path = slot.path.trim();
      const nextSlot: LauncherSlot = {
        ...slot,
        path,
        dataType,
        exist: await pathExists(path, dataType),
      };
      await saveSlot(nextSlot);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <main className="settings-page" data-theme={settings.theme}>
      <div className="settings-card">
        <h1>ショートカット設定 {slotIndex + 1}</h1>
        <div className="form-row">
          <label>Path</label>
          <input
            value={slot.path}
            onChange={(event) => setSlot((current) => ({ ...current, path: event.target.value }))}
            placeholder="C:\\path\\to\\target"
          />
        </div>
        <div className="form-row">
          <label>Arguments</label>
          <input
            value={slot.arg}
            onChange={(event) => setSlot((current) => ({ ...current, arg: event.target.value }))}
            placeholder="Optional arguments"
          />
        </div>
        <div className="form-row">
          <label>Comment</label>
          <textarea
            value={slot.comment}
            onChange={(event) => setSlot((current) => ({ ...current, comment: event.target.value }))}
            placeholder="Comment to display in tooltip"
          />
        </div>
        {error ? <div className="message">{error}</div> : null}
        <div className="settings-actions" style={{ justifyContent: 'space-between' }}>
          <button type="button" className="danger-button" onClick={() => void handleClear()}>
            Clear
          </button>
          <div style={{ display: 'flex', gap: 8 }}>
            <button type="button" className="secondary-button" onClick={() => void closeWindow()}>
              Cancel
            </button>
            <button type="button" className="primary-button" onClick={() => void handleSave()}>
              Save
            </button>
          </div>
        </div>
      </div>
    </main>
  );
}
