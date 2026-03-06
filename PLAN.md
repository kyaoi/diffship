# PLAN (diffship OS)

このファイルは、diffship を『AIを用いた開発OS』として育てるための進捗管理の唯一の正です。
チャットを切り替えても復帰できるように、**現在地・次にやること・完了条件**をここに集約します。

## 関連ドキュメント

- 仕様: `docs/SPEC_V1.md`
- Patch bundle 契約: `docs/PATCH_BUNDLE_FORMAT.md`
- Project kit テンプレ: `docs/PROJECT_KIT_TEMPLATE.md`
- 設定: `docs/CONFIG.md`
- トレーサビリティ: `docs/TRACEABILITY.md`
- 意思決定ログ: `docs/DECISIONS.md`

---

## ゴール

ユーザーが基本的に何も気にせず、

```bash
# 1) handoff（diff → AI bundle）
diffship build [options...]

# 2) ops（AI patch bundle → apply/verify/promote）
diffship loop <patch-bundle.zip>
```

を回せる状態を作る。

### 成立させたいこと（Ops 側）
- **作業ツリーを汚さず**（worktree / session / sandbox）
- **verify を回し**（fast/standard/full）
- 成功したら **自動で promotion（commit）**
- 危険（secrets / 要ユーザー作業）なら **必ず止まって警告**

### 成立させたいこと（Handoff 側）
- Git の差分（committed/staged/unstaged/untracked）を **アップロード制限に合わせて分割**し、
  **AI が読める入口（HANDOFF.md）**付きの bundle にする
- `.diffshipignore` / secrets warning を尊重し、**危険/巨大/バイナリ**は除外 or attachments として扱う

---

## 公式デフォルト（V1）

- OS mode: isolated worktrees (session + sandbox)
- Promotion: `commit`
- Commit policy: `auto`
- Verify profile: `standard`
- Safety: clean-tree必須 / base_commit一致必須 / path guard 有効 / lock 有効

---

## 運用ルール（保険）

- 進捗更新は必ずこの `PLAN.md` を更新する
- 重要な意思決定（デフォルト変更・安全ポリシー変更）は `docs/DECISIONS.md` に追記
- 仕様変更を入れたら、同一コミットで `docs/SPEC_V1.md` と `docs/TRACEABILITY.md` も更新する
- 変更後は必ず通す:
  - `just docs-check`
  - `just trace-check`

---

## Status 定義

- `todo`: 未着手
- `doing`: 着手中
- `blocked`: 障害あり（理由を書く）
- `done`: 完了

---

## Milestones

### M0: OSの背骨（init / lock / runs）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M0-01 | done | `diffship init`（project kit生成） | `.diffship/` が生成され、既存があれば安全にスキップ/`--force`で上書き |
| M0-02 | done | ロック（同時実行防止） | .diffship/lock が作られ、二重起動を拒否できる |
| M0-03 | done | runsの保存（run-id/ログ） | `.diffship/runs/<run-id>/run.json` が作られ、少なくとも `init` の結果（`init.json`）が保存される（apply/verify は M2 で拡張） |
| M0-04 | done | M0の統合テスト | 一時git repo上で `init`→`status`→`runs` が通る |

### M1: worktree/session/sandbox（作業ツリーを汚さない核）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M1-01 | done | session 作成/再利用 | .diffship/worktrees/ 配下の session を安定して再利用できる |
| M1-02 | done | sandbox 作成（runごと） | runs（run-id）と対応する sandbox を作れる |
| M1-03 | done | クリーンアップ方針 | 失敗/中断時でも破綻せず `status` で復旧できる |

### M2: apply → verify → promotion（commit）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M2-01 | done | patch bundle 検証（構造/manifest/path） | 不正bundleを確実に拒否できる |
| M2-02 | done | `apply`（sandboxで） | apply成功/失敗がrunに記録され、失敗時はrollbackされる |
| M2-03 | done | `verify`（standard） | profileでチェックが走り、summaryがrunに保存される |
| M2-04 | done | promotion=commit | verify成功時に commit が作られる（messageはbundle由来） |
| M2-05 | done | `loop`（M2結合） | `diffship loop` で成功→commit まで完走 |
| M2-06 | done | pack-fix（verify失敗時） | `loop` で verify失敗したら自動で reprompt zip を作る |

### M3: secrets / tasks（止めるべき時に止まる）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M3-01 | done | secrets 検知 → promotion停止 | 危険検知時に必ず止まり、明示ackがないと promoteできない |
| M3-02 | done | tasks 同梱契約 | bundleの tasks/USER_TASKS.md が run に残り、ユーザーが実行すべき作業が見える |

### M4: 設定（グローバル/プロジェクト/CLI/bundle）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M4-01 | done | 設定ロード優先順位 | CLI > manifest > project > global > default の順で確定する |
| M4-02 | done | commit/promotion切替 | `--promotion` / `--commit-policy` で挙動を切り替えられる |

---


### M5: TUI（操作の見える化 + 実行支援）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M5-01 | done | TUI骨格（起動/終了/画面遷移） | `diffship`（引数なし）でTUIが起動し、q/ESCで安全に終了できる。非TTYでは従来通りヘルプを出す。 |
| M5-02 | done | Read-only: status/runs ビューア | `status`/`runs` 相当の情報を一覧でき、run詳細（apply/verify/promotion）とエラー/exit code が確認できる。 |
| M5-03 | done | Runアーティファクト導線（paths/tasks） | run dir / tasks/USER_TASKS.md などのパスを画面上で明示し、コピー/参照しやすい導線を用意する（最低限: 表示）。 |
| M5-04 | done | Action: TUIから `loop` を実行 | TUIから bundle を指定して `loop` を起動でき、進捗/結果（成功/失敗/停止理由）を表示できる（実処理は既存opsを呼ぶ）。 |
| M5-05 | done | CLI parity / テスト（CI green） | TUIはCLIの薄いラッパに徹し、主要操作の結果がCLIと一致する。最低限の非TTYスモークテストを追加し、`clippy -D warnings` を通す。 |



### M6: Handoff（diff → AI bundle）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M6-01 | done | `diffship build`（handoff bundle生成） | `diffship build --help` があり、最小指定で bundle を生成できる。出力レイアウトが `docs/BUNDLE_FORMAT.md` に一致する。 |
| M6-02 | done | diff収集（committed/staged/unstaged/untracked） | committed/staged/unstaged/untracked を CLI で選択でき、各segmentの基準（committed range / HEAD 等）を `HANDOFF.md` に記録できる。 |
| M6-03 | todo | 分割（profiles）+ excluded/attachments | profile制限内で `parts/part_XX.patch` を分割できる。超過・除外は `excluded.md`、raw添付は `attachments.zip` に退避できる。 |
| M6-04 | done | `HANDOFF.md` 生成（入口） | `docs/HANDOFF_TEMPLATE.md` の構造に沿って TL;DR / change map / parts index を生成できる。 |
| M6-05 | todo | ignore + secrets warning（handoff側） | `.diffshipignore` を尊重し、secrets らしき内容は値を出さずに警告できる（必要なら fail も可能）。 |
| M6-06 | todo | determinism + テスト | 出力の順序/分割が決定的で、goldenテストを用意し `just ci` が通る。 |

---
## Next（いま着手する3つ）

1) M6-03: profile 制限に合わせた split / excluded / attachments
2) M6-05: `.diffshipignore` + handoff secrets warning（値を出さない/CI用fail）
3) M6-06: handoff 出力の golden / determinism テストを増やす

（候補）
- Handoff `preview`
- Handoff TUI（インタラクティブ選択）
- verify の config-driven profiles（`[verify.profiles.*]`）強化


## メモ（詰まったらここに書く）

- blocked理由、調査ログ、設計メモなど

- M6-01 のフォローアップ（CI fix）:
  - `tests/m6_handoff_build.rs` は predicates の `eval` を使わずに `str::contains` に統一（trait import漏れ/依存増を避ける）
  - `tests/m6_handoff_build.rs` は default branch 名（main/master）を仮定せず、`rev-parse --abbrev-ref HEAD` で取得したブランチ名を使う
  - `src/handoff.rs` は `clippy::too-many-arguments` を避けるため `HandoffDocInputs` に集約

- Zip overlay を展開するとファイルの更新時刻が戻り、Cargo が再ビルドしないことがある。
  - サブコマンドが認識されない等の症状が出たら `cargo clean` → `just ci` を試す。

- M6-02 実装メモ:
  - `diffship build` は `--include-staged` / `--include-unstaged` / `--include-untracked` と `--no-committed` を受け付ける
  - untracked は現時点では text add-diff のみ。binary/unreadable は File Table に skip note を残し、attachments は M6-03 で扱う
  - `docs/TRACEABILITY.md` の `Status: Partial` は、Tests か Code のどちらかに `TBD` が残る場合だけ使う（両方埋まっていれば `Implemented`）
