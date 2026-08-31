export type DataType =
  | 'none'
  | 'folder'
  | 'exe'
  | 'script'
  | 'url'
  | 'text'
  | 'image'
  | 'doc'
  | 'cpl'
  | 'otherApp';

export interface LauncherSlot {
  index: number;
  dataType: DataType;
  path: string;
  arg: string;
  comment: string;
  exist: boolean;
}

export interface LauncherSettings {
  version: number;
  slots: LauncherSlot[];
}

export const SLOT_COUNT = 64;

export function createEmptySlot(index: number): LauncherSlot {
  return {
    index,
    dataType: 'none',
    path: '',
    arg: '',
    comment: '',
    exist: false,
  };
}

export function createDefaultSettings(): LauncherSettings {
  return {
    version: 1,
    slots: Array.from({ length: SLOT_COUNT }, (_, index) => createEmptySlot(index)),
  };
}
