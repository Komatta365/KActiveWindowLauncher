# ADR 0001: Tauri v2 + Next.js + React 構成を採用する

- Status: Accepted
- Date: 2026-08-31

## Context
- 移植元は WinForms + C# で実装された Windows 向けランチャーである。
- 次期実装では、UI 開発効率と保守性を高めつつ、ネイティブ連携（Win32 API）を継続して利用する必要がある。
- 要求技術として TypeScript, Rust, Tauri v2, Next.js, React, shadcn/ui, Tailwind CSS, Material Symbols, dnd-kit が指定されている。

## Decision
以下を正式採用する。

1. デスクトップ基盤: **Tauri v2 + Rust**
   - ネイティブ機能（ウィンドウ制御、プロセス起動、永続化）を Rust 側で実装する。
2. UI 基盤: **Next.js + React + TypeScript**
   - 画面構成と状態管理を React コンポーネントで実装する。
3. UI コンポーネント/スタイル:
   - **shadcn/ui**（再利用コンポーネント）
   - **Tailwind CSS**（ユーティリティベースのスタイリング）
   - **Material Symbols**（アイコン）
4. DnD:
   - **dnd-kit** を使用してスロット並べ替え・ドロップ登録を実装する。

## Consequences

### Positive
- Web 技術で UI を高速に実装・改善できる。
- Tauri により軽量なデスクトップ配布が可能になる。
- Rust 側に OS 依存処理を集約し、責務分離しやすい。

### Negative / Trade-off
- フロントエンド（TypeScript）とバックエンド（Rust）の二言語運用になる。
- Win32 依存機能は Rust 側実装・検証コストが発生する。
- Next.js をデスクトップアプリ文脈で運用するため、ビルド/配布手順の整備が必要。

## Notes
- 本ADRは「まず同等移植」を前提とし、新規機能追加は後続ADRで判断する。
