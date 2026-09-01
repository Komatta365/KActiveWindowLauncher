'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { createDefaultSettings, type LauncherSettings, type ThemeType } from '@/lib/launcher';

export default function SettingsPage() {
  const [settings, setSettings] = useState<LauncherSettings>(createDefaultSettings());
  const [theme, setTheme] = useState<ThemeType>('classic');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const loaded = await invoke<LauncherSettings>('settings_load');
        setSettings(loaded);
        setTheme(loaded.theme);
      } catch (e) {
        setError(String(e));
      }
    };
    void load();
  }, []);

  const closeWindow = async () => {
    await getCurrentWindow().close();
  };

  const save = async () => {
    try {
      const nextSettings = { ...settings, theme };
      await invoke('settings_save', { settings: nextSettings });
      await closeWindow();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <main className="settings-page">
      <div className="settings-card">
        <h1>設定</h1>
        <div className="form-row">
          <label>テーマ</label>
          <select value={theme} onChange={(event) => setTheme(event.target.value as ThemeType)}>
            <option value="classic">Classic</option>
            <option value="dark">Dark</option>
            <option value="light">Light</option>
          </select>
        </div>
        {error ? <div className="message">{error}</div> : null}
        <div className="settings-actions">
          <button type="button" className="secondary-button" onClick={() => void closeWindow()}>
            Cancel
          </button>
          <button type="button" className="primary-button" onClick={() => void save()}>
            Save
          </button>
        </div>
      </div>
    </main>
  );
}
