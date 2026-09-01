# KActiveWindowLauncher

Windows 向けのアクティブウィンドウ追従型ランチャーを、Tauri + React + Rust で移植・実装するためのリポジトリです。

## 概要

本リポジトリは既存の ActiveWindowLauncher を、モダンなデスクトップアプリとして再実装するための設計・仕様・実装方針をまとめたプロジェクトです。

- UI は Next.js / React / shadcn/ui / Tailwind を前提とします。
- ネイティブ処理は Rust / Tauri v2 を利用します。
- 実装の背景や設計判断は ADR と仕様書で管理し、追跡しやすくしています。

## フォルダ構成

```text
KActiveWindowLauncher/
├── .github/                          # GitHub 関連の設定・運用ファイル
├── .vscode/                          # VS Code 向けワークスペース設定
├── doc/                              # プロジェクト資料の置き場
│   ├── adr/                          # Architecture Decision Record
│   │   ├── 0001-adopt-tauri2-nextjs-react-stack.md
│   │   ├── 0002-adopt-frontend-ui-stack.md
│   │   └── 0003-adopt-dnd-kit-drag-overlay.md
│   └── spec/                         # 実装・移植仕様書
│       └── active-window-launcher-migration-spec.md
├── KRootMark                         # プロジェクトのルート状態・マーカー用ファイル
├── KActiveWindowLauncher.code-workspace # VS Code のワークスペース定義
├── README.md                         # この説明書
└── .gitignore                        # Git 管理対象外設定
```

## 各フォルダの役割

- .github/
  - GitHub の issue / workflow / contribution 関連設定を置くための領域です。
- .vscode/
  - エディタ設定やワークスペース固有の環境設定を管理します。
- doc/
  - 実装方針、設計判断、移植仕様を記録するドキュメント置き場です。
- doc/adr/
  - 技術選定の理由や背景を残す ADR を格納します。
- doc/spec/
  - 機能要件、画面構成、データ構造、イベント仕様などの詳細仕様を管理します。
- KRootMark
  - ルート情報やプロジェクト識別に利用するマーカーです。

## ドキュメントの参照先

- 実装方針の要点は [doc/adr](doc/adr) を確認してください。
- 機能要件や移植仕様は [doc/spec/active-window-launcher-migration-spec.md](doc/spec/active-window-launcher-migration-spec.md) を参照してください。

## ビルド / 起動バッチ

リポジトリ直下で、以下のバッチファイルを利用できます。

- デバッグビルド
  - `build-kactive-window-launcher-debug.bat`
  - 実行内容: `npm run tauri:build -- --debug`
- リリースビルド
  - `build-kactive-window-launcher-release.bat`
  - 実行内容: `npm run tauri:build`
- デバッグ起動
  - `run-kactive-window-launcher-debug.bat`
  - 実行内容: `npm run tauri:dev`
- リリース起動
  - `run-kactive-window-launcher-release.bat`
  - `src-tauri\target\release\app.exe` が存在しない場合は、先に `build-kactive-window-launcher-release.bat` を実行してから起動します。

## 今後の方針

このリポジトリは、設計・仕様の整理を先行させたうえで、実装フェーズに進める構成です。各種設計書をもとに、Tauri アプリの実装を段階的に進めていく想定です。
