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
  - ユーザーがやるべき作業は bundle の `tasks/USER_TASKS.md` に同梱する
