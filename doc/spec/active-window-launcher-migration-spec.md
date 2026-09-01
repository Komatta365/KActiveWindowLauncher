# ActiveWindowLauncher 移植 実装詳細仕様

## 1. 目的
- 既存 WinForms 版 ActiveWindowLauncher を、**TypeScript + Rust + Tauri v2** で機能同等移植する。
- 実装時の解釈差異を防ぐため、UI/バックエンド/API/データ/状態遷移/検証を詳細定義する。

## 2. 前提
- 移植元: `D:\repository\Bitbucket\mytools\ForWindows\Launchers\ActiveWindowLauncher`
- 対応 OS: Windows（第1フェーズ）
- UI 技術: Next.js, React, shadcn/ui, Tailwind CSS, Material Symbols, dnd-kit
- ネイティブ処理: Rust（Tauri v2 コマンド + イベント）

## 3. 実装スコープ
### 3.1 対象
1. アクティブウィンドウ追従バー
2. スロット（固定 64 件）
3. 登録/編集/削除/実行
4. スロット並べ替え（DnD）
5. ファイル/フォルダ/URL ドロップ登録
6. ツールチップ表示
7. タスクトレイ格納・再表示・終了
8. 設定永続化（復旧用バックアップ含む）

### 3.2 非対象
- 新規機能追加（タグ、検索、クラウド同期等）
- Windows 以外のネイティブ実装

## 4. 用語・型定義
### 4.1 DataType
```ts
type DataType =
  | "none"
  | "folder"
  | "exe"
  | "script"
  | "url"
  | "text"
  | "image"
  | "doc"
  | "cpl"
  | "otherApp";
```

### 4.2 スロットデータ
```ts
interface LauncherSlot {
  index: number;      // 0..63
  dataType: DataType;
  path: string;       // 空文字許容
  arg: string;        // 空文字許容
  comment: string;    // 空文字許容
  exist: boolean;     // 実体存在判定結果
}
```

### 4.3 設定全体
```ts
interface LauncherSettings {
  version: number;
  slots: LauncherSlot[]; // 常に length=64
}
```

## 5. アーキテクチャ
## 5.1 フロントエンド責務（Next.js/React）
- スロットバー描画、ホバー状態、選択状態
- 編集ダイアログ表示と入力バリデーション
- dnd-kit による並べ替え UI とドロップ受付
- Tauri コマンド呼び出しとイベント反映

### 5.2 Rust/Tauri責務
- Win32 API でアクティブウィンドウ監視・追従座標計算・Zオーダー制御
- 実行（ShellExecuteW/CreateProcessW 相当）
- 設定ファイル read/write + バックアップローテーション
- エラーを文字列で返却（握り潰し禁止）

## 6. 機能仕様（実装詳細）
### FR-01 ウィンドウ追従
1. 100ms 間隔で監視 tick を実行する。
2. `GetForegroundWindow` 取得結果から除外判定:
   - 自アプリウィンドウ
   - 自アプリ子ウィンドウ
   - タスクトレイ
   - ツールウィンドウ/子ウィンドウ/Caption無しなど不適合ウィンドウ
3. 追従先が有効な場合、対象矩形上辺へバーを配置する。
4. 追従先が無効な場合、バーを非表示（opacity 0 / hidden）にする。
5. 追従処理は Rust 側で実行し、`launcher://window-tracked` を毎 tick 通知する。
6. 配置時は追従対象ウィンドウの横幅にランチャー横幅を一致させ、上辺に吸着させる。

### FR-02 ロック
- `isLocked=false`:
  - 規定フレーム数（3）連続で対象が変わった時のみ参照先更新（ちらつき抑制）
- `isLocked=true`:
  - 参照先を固定し、追従先変更を停止する
  - ランチャーは参照ウィンドウ上辺への配置は継続する

### FR-03 スロット管理
- スロット数は 64 固定。
- 起動時、設定不在時は `dataType=none` で 64 件初期化。
- index は UI 配列順と常に一致させる。

### FR-04 実行
- 左クリック:
  - `dataType != none` かつ `exist=true` の場合のみ実行
  - それ以外はショートカット編集ダイアログ（独立ウィンドウ）を開く
- 右クリック:
  - 常にショートカット編集ダイアログ（独立ウィンドウ）を開く
- 既存スロットへのドロップ実行引数:
  - ドロップパスに空白がある場合、二重引用符でラップして渡す
- DataType 別実行:
  - folder: フォルダ起動
  - exe/script/otherApp/text/image/doc: アプリケーション実行
  - url: 既定ブラウザで開く
  - cpl: コントロールパネル起動

### FR-05 DnD
- スロットからスロットへドロップ:
  - 同一 index: 実行
  - 別 index: 内容入れ替え
- ファイル/フォルダ/URL ドロップ:
  - 空スロットなら登録
  - 登録済みスロットなら引数実行

### FR-06 ツールチップ
- ホバー中スロットが `dataType != none` の場合、
  - `path + "\n" + comment` を表示
- 対象外に移動で非表示

### FR-07 タスクトレイ
- Close ボタン押下は終了せず `tray` へ格納
- ウィンドウ右上の閉じる操作も終了せず `tray` へ格納
- トレイアイコン左クリックでメニューを表示（表示 / 非表示 / 設定 / 終了）
- トレイアイコンダブルクリックで再表示
- トレイメニュー「終了」でアプリ終了

### FR-09 設定ダイアログ
- トレイメニュー「設定」で設定ダイアログを表示する
- 設定ダイアログでテーマを選択・保存できる
- テーマは `classic | dark | light` をサポートし、設定保存で永続化する

### FR-08 永続化・復旧
1. 起動時:
   - メイン設定ファイルが有効なら読込
   - 破損/空ファイルなら日付付きバックアップを新しい順で探索
2. 終了時:
   - メイン設定保存
   - 日付付きバックアップ上書き保存
3. バックアップ保持:
   - 最大 5 世代（古い順に削除）

## 7. DataType 判定仕様
1. `Directory.Exists(path)` なら `folder`
2. `^(http://|https://|www)`（大文字小文字無視）なら `url`
3. 拡張子判定:
   - `.exe` => `exe`
   - `.vbs`, `.bat` => `script`
   - `.bmp`, `.jpg`, `.jpe`, `.jpeg`, `.png`, `.tga` => `image`
   - `.txt` => `text`
   - `.htm`, `.html` => `url`
   - `.ppt`, `.xls`, `.doc`, `.pdf` => `doc`
   - `.cpl` => `cpl`
   - その他 => `otherApp`

## 8. Tauri コマンドI/F（確定仕様）
実装時は下記シグネチャを満たすこと。

```ts
// 初期化・監視
invoke("launcher_init"): Promise<void>
invoke("launcher_start_tracking"): Promise<void>
invoke("launcher_stop_tracking"): Promise<void>

// 設定
invoke("settings_load"): Promise<LauncherSettings>
invoke("settings_save", { settings }: { settings: LauncherSettings }): Promise<void>

// スロット操作
invoke("slot_swap", { lhs, rhs }: { lhs: number; rhs: number }): Promise<LauncherSettings>
invoke("slot_update", { slot }: { slot: LauncherSlot }): Promise<LauncherSettings>
invoke("slot_clear", { index }: { index: number }): Promise<LauncherSettings>

// 実行
invoke("slot_execute", {
  index,
  droppedArg
}: {
  index: number;
  droppedArg?: string;
}): Promise<void>

// 補助
invoke("path_detect_data_type", { path }: { path: string }): Promise<DataType>
invoke("path_exists", { path, dataType }: { path: string; dataType: DataType }): Promise<boolean>
```

### 8.2 現行実装での監視・ロック意味
- `launcher_init`
  - Win32 監視スレッドを起動する（100ms tick）
  - 起動後は常時監視を継続する
- `launcher_start_tracking`
  - ロック状態へ遷移 (`locked=true`)
  - 参照ウィンドウの変更を止める
- `launcher_stop_tracking`
  - アンロック状態へ遷移 (`locked=false`)
  - 規定フレーム数判定で参照ウィンドウ追従を再開する

### 8.1 フロントへ通知するイベント
- `launcher://window-tracked`
  - payload: `{ x: number; y: number; width: number; visible: boolean }`
- `launcher://reference-window-changed`
  - payload: `{ hwnd: string | null; locked: boolean }`
- `launcher://tracking-error`
  - payload: `{ message: string }`

## 9. UI構成（実装単位）
- `LauncherBar`:
  - 追従位置適用、スクロール状態、システムボタン（Lock/Left/Right/Close）
- `LauncherSlotGrid`:
  - 64スロットの可視範囲描画
- `LauncherSlotItem`:
  - アイコン、存在不正時表示、ホバー状態
- `SlotEditDialog`:
  - パス/引数/コメント編集、削除、親フォルダを開く
- `TrayMenuBridge`:
  - トレイイベントを React 状態へ中継

## 10. 状態遷移
### 10.1 アプリ全体
- `booting` -> `ready` -> (`tracking` <-> `trayHidden`) -> `quitting`

### 10.2 スロット
- `empty` -> `configured` -> `invalidTarget`（ファイル消失時）
- `invalidTarget` でも編集は可能、実行は不可

## 11. エラー処理方針
- Rust コマンド失敗は `Result::Err(String)` で返し、UIで通知表示する。
- 失敗時に成功扱いで黙殺しない。
- 設定ファイル破損時は復旧対象ファイル名を表示する。

## 12. 実装順序（推奨）
1. 型定義と settings load/save
2. スロット表示と編集ダイアログ
3. 実行コマンド
4. dnd-kit 並べ替え
5. ファイル/URL ドロップ
6. Win32 追従と lock
7. タスクトレイ連携
8. バックアップ/復旧

## 13. 受け入れ基準（完了定義）
1. 64スロットを登録/編集/削除/実行できる
2. スロット並べ替えとドロップ登録が動作する
3. アクティブウィンドウ追従とロックが再現される
4. タスクトレイ格納・復帰・終了が動作する
5. 設定保存/再起動復元/バックアップ復旧が動作する
6. 主要失敗系（不正パス、破損設定、実行失敗）で通知される

## 14. テスト観点（最小必須）
- 単体:
  - DataType 判定
  - スロット入れ替え
  - 保存データの正規化（64件補正）
- 結合:
  - settings load/save + backup rotation
  - slot_execute の DataType 別分岐
- E2E（Windows）:
  - 追従表示、ロック、トレイ復帰
  - ファイルドロップ登録、URLドロップ登録
