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
diffship loop <patch-bundle.zip>
```

を繰り返すだけで、

- **作業ツリーを汚さず**（worktree / session / sandbox）
- **verify を回し**（fast/standard/full）
- 成功したら **自動で promotion（commit）**
- 危険（secrets / 要ユーザー作業）なら **必ず止まって警告**

が成立する状態を作る。

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
| M2-06 | todo | pack-fix（verify失敗時） | `loop` で verify失敗したら自動で reprompt zip を作る |

### M3: secrets / tasks（止めるべき時に止まる）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M3-01 | done | secrets 検知 → promotion停止 | 危険検知時に必ず止まり、明示ackがないと promoteできない |
| M3-02 | todo | tasks 同梱契約 | bundleの tasks/USER_TASKS.md が run に残り、ユーザーが実行すべき作業が見える |

### M4: 設定（グローバル/プロジェクト/CLI/bundle）

| ID | Status | 内容 | Done条件 |
|---|---|---|---|
| M4-01 | todo | 設定ロード優先順位 | CLI > manifest > project > global > default の順で確定する |
| M4-02 | todo | commit/promotion切替 | `--promotion` / `--commit-policy` で挙動を切り替えられる |

---

## Next（いま着手する3つ）

1) M3-02 tasks 同梱契約
2) M2-06 pack-fix（verify失敗時の reprompt zip）
3) M4-01 設定ロード優先順位（CLI > bundle > project > global > default）

---

## メモ（詰まったらここに書く）

- blocked理由、調査ログ、設計メモなど

- Zip overlay を展開するとファイルの更新時刻が戻り、Cargo が再ビルドしないことがある。
  - サブコマンドが認識されない等の症状が出たら `cargo clean` → `just ci` を試す。
