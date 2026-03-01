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
