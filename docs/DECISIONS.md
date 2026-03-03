# Decisions (diffship OS)

diffship OS の重要な意思決定ログです。
チャットを切り替えても『何を決めたか』を失わないために、**結論だけを短く**残します。

---

## D-001: OSとして最強に寄せる（worktree/session/sandbox）

- Date: 2026-03-01
- Decision:
  - ops は isolated worktrees（session + sandbox）で実行し、ユーザーの作業ツリーを汚さない
- Rationale:
  - “apply を繰り返すだけ”の運用を成立させるため
- Implications:
  - worktree 管理・クリーンアップ・ロックが必須

---

## D-002: 公式デフォルト（V1）

- Date: 2026-03-01
- Defaults:
  - Promotion: `commit`
  - Commit policy: `auto`
  - Verify profile: `standard`
  - Safety: clean-tree必須 / base_commit一致必須 / path guard / lock

---

## D-003: git-apply / git-am と commit を分離

- Date: 2026-03-01
- Decision:
  - apply_mode（パッチ形式）と commit_policy（コミット方針）を分離して両対応する
- Notes:
  - `commit_policy=auto` の場合、`apply_mode=git-apply` でも `git commit -F` でコミット可能

---

## D-004: secrets / 要ユーザー作業は勝手に進めない

- Date: 2026-03-01
- Decision:
  - secrets らしきものを検知したら promotion を停止し、ユーザーの明示 ack が必要
  - ユーザーがやるべき作業は bundle の tasks/USER_TASKS.md に同梱する

---

## D-005: worktree レイアウトと復旧戦略（status で復帰可能）

- Date: 2026-03-01
- Decision:
  - worktree は `.diffship/worktrees/` 配下に集約する
    - session worktree: `.diffship/worktrees/sessions/<session>/`
    - sandbox worktree: `.diffship/worktrees/sandboxes/<run-id>/`
  - session state は以下を組み合わせて保持する
    - git ref: `refs/diffship/sessions/<session>`
    - state json: `.diffship/sessions/<session>.json`
  - run と sandbox の紐付けは run dir に保存する
    - `.diffship/runs/<run-id>/sandbox.json`
- Recovery:
  - `diffship status` は sessions と sandboxes を列挙し、
    - 中断時に「どの run の sandbox が残っているか」を確認できる
    - 必要に応じて `git worktree remove --force <path>` で復旧/掃除できる
  - sandbox 削除は best-effort（成功/失敗どちらでも落ちない）を基本とし、
    - 取りこぼしは `status` で可視化して回収する

---

## D-006: verify profile のデフォルト（M2）

- Date: 2026-03-01
- Decision:
  - `verify` は「ローカルで定義されたコマンド」を実行する（bundle内のコマンドは実行しない）
  - M4（設定ロード）実装前の暫定として、以下のヒューリスティクスをデフォルトにする
    1) `justfile` があり `just` が利用可能 → profile に応じた `just ...` を実行
    2) `Cargo.toml` がある → profile に応じた `cargo ...` を実行
    3) それ以外 → `git diff --check` のみ実行
- Rationale:
  - diffship 自身（このリポジトリ）では `just` を品質ゲートとして使う
  - 一方で、任意の Git repo でも `verify` 自体は破綻しないようにする

---

## D-007: promotion=commit の実装方針（M2-04）

- Date: 2026-03-01
- Decision:
  - promotion は sandbox の結果を **target branch に cherry-pick** して反映する
  - `apply_mode=git-apply` の場合は sandbox 上で `git commit -F` により **1コミット**を作ってから cherry-pick する
  - promotion の安全条件として、**target branch の HEAD が sandbox の base_commit と一致**しない場合は拒否する
- Defaults:
  - target branch は `develop` を優先し、存在しない場合は現在のブランチへフォールバックする
- Artifacts:
  - `.diffship/runs/<run-id>/promotion.json` に promotion 結果を保存する

---

## D-008: loop の実装方針（M2-05）

- Date: 2026-03-01
- Decision:
  - `loop` は 1つのロックを保持したまま `apply` → `verify` → `promote` を実行する
  - M2 段階では `loop` 用の run-id は `apply` の run-id を利用し、同 run dir に `verify.json` / `promotion.json` を追記する
  - verify 失敗時は `.diffship/runs/<run-id>/pack-fix.zip`（reprompt kit）を生成し、sandbox を残して終了する

---

## D-009: 未来用の exit code 定数は dead_code を許可して保持

- Date: 2026-03-02
- Decision:
  - SPEC に先行して exit code を予約する場合、実装が入るまで `#[allow(dead_code)]` を付けて保持する（`-D warnings` 対策）
- Notes:
  - 予約コードを消すと SPEC/実装の整合が崩れやすいので、予約コードは残す

---

## D-010: tasks は promotion を止め、--ack-tasks を要求する

- Date: 2026-03-03
- Decision:
  - bundle に `tasks/USER_TASKS.md` が存在する場合、promotion はデフォルトで停止し、ユーザーの明示 `--ack-tasks` が必要
- Rationale:
  - 手動作業（env作成/鍵更新/移行など）を飛ばすと壊れるため
- Implications:
  - `diffship apply` は tasks のパスを出力し、run dir に保持する
  - `diffship promote/loop` は tasks 未ack時に exit=12 で拒否する

---

## D-011: 設定ロードの優先順位（M4-01）

- Date: 2026-03-03
- Decision:
  - 設定は以下の優先順位で **マージ** して確定する
    - CLI > patch bundle manifest > project > global > default
  - project 設定は `.diffship/config.toml` を正とし、互換のため `./.diffship.toml` も読み取る（同一キーがあれば `.diffship/config.toml` が勝つ）
  - global 設定は `~/.config/diffship/config.toml`
  - bundle 側の上書きは `manifest.yaml` の以下の任意キーで行う
    - `verify_profile`, `target_branch`, `promotion_mode`, `commit_policy`
- Rationale:
  - その場の run（bundle）で明示的に挙動を上書きしつつ、普段は project/global のデフォルトで回せるようにする
- Notes:
  - 現状の実装で実際に参照するキーは段階的に増やす（未使用キーは無視される）
