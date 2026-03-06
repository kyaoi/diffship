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
  - verify 失敗時は `pack-fix`（reprompt zip）を作成して終了する（M2-06）

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

## D-011: 設定ロードの優先順位（config precedence）

- Date: 2026-03-03
- Decision:
  - 設定の最終値は以下の優先順位で決める（後勝ち）
    1) CLI flags
    2) bundle manifest
    3) project config（`.diffship/config.toml`）
    4) global config（`~/.config/diffship/config.toml`）
    5) built-in defaults
- Notes:
  - “未指定”は上位で上書きしない（Option は None のまま次の層へ委譲）
  - これにより「普段のデフォルトを保ちつつ run 単位で安全にオーバーライド」が可能

---

## D-012: M4-02 CLI flags で promotion/commit-policy を切り替える

- Date: 2026-03-03
- Decision:
  - `--promotion`（none|working-tree|commit）と `--commit-policy`（auto|manual）で挙動を切り替えられるようにする
  - 設定優先順位は D-011 に従い、CLI は最優先で上書きする
- Rationale:
  - bundle / project / global のデフォルトを保ちつつ、run 単位で安全にオーバーライドしたい
- Notes:
  - `promotion=none` の場合は promotion をスキップし、run に promotion.json を残す（sandbox は既定で保持）
  - `commit-policy=manual` の場合、git-apply の promotion は sandbox 側にコミットが存在することを要求する

---

## D-013: TUI v0 は「可視化 + 実行支援」に絞り、CLI とパリティを保つ

- Date: 2026-03-04
- Decision:
  - TUI は「status/runs の可視化」と「loop 実行の支援」に絞る（まずは read-only 中心）
  - TUI は既存 ops/CLI を呼び出す薄いラッパとして実装し、**TUIだけにしかない挙動**を作らない（CLI parity）
  - 起動導線は `diffship tui` を用意しつつ、`diffship`（引数なし）を TTY のときは TUI 起動に寄せる（非TTYは従来のヘルプ/エラー）
- Rationale:
  - 自動化（CLI/loop）を壊さずに、運用中の「いま何が起きたか」を見える化したい
- Implications:
  - 画面は「Runs」「Status」「Run detail/log」「Loop」の最小セットから始める

---

## D-014: raw mode の描画は CRLF に寄せて端末差を吸収する

- Date: 2026-03-04
- Decision:
  - crossterm の raw mode 下では `\n` のみだと行頭に戻らず描画が崩れる端末があるため、
    TUI の 1行出力は `\r\n`（CRLF）で明示的に改行する。
- Rationale:
  - “次の行が前行の続き列から始まる”崩れを確実に防ぐため。
- Implications:
  - 出力ヘルパ（writeln_trunc）は CRLF を使う前提で統一する。

---

## D-015: TUI 起動のデフォルトは TTY のみ、環境変数で無効化できる

- Date: 2026-03-04
- Decision:
  - `diffship`（引数なし）は **TTY のときのみ** TUI を起動する（非TTYは従来どおりヘルプ/エラー）。
  - `diffship tui` を明示サブコマンドとして用意する。
  - `DIFFSHIP_NO_TUI=1` 等で自動TUIを無効化できる。
- Rationale:
  - CI / パイプ / スクリプト実行を壊さず、対話時だけ可視化したい。
- Implications:
  - “TUIにしかない挙動”を持たない（CLI parity を保つ）。

---

## D-016: TUI の CLI parity は非TTY前提のスモークテストで固定する

- Date: 2026-03-04
- Decision:
  - テストでは非TTY環境で、
    - `diffship`（引数なし）は help を出して即終了する
    - `diffship tui` は “requires a TTY” で失敗する
    を確認する。
  - ハング防止のため timeout を必ず付ける。
- Notes:
  - `assert_cmd::Command::cargo_bin` は deprecated（custom build-dir 非互換）なので `assert_cmd::cargo::cargo_bin_cmd!` を使う。

---

## D-017: `-D warnings` 運用下ではテストの import も最小化する

- Date: 2026-03-04
- Decision:
  - `assert_cmd::prelude::*` のような未使用 import を避け、必要なものだけを import する。
- Rationale:
  - `-D warnings` で CI が落ちるのを防ぐ（テストも同じ品質ゲートを通す）。

---
## D-018: Handoff は `diffship build` から実装を開始し、まずは committed-only を MVP にする

- Date: 2026-03-05
- Decision:
  - Handoff bundle の生成コマンドは `diffship build` とし、出力は `docs/BUNDLE_FORMAT.md` / `docs/HANDOFF_TEMPLATE.md` に準拠する
  - 初期MVPは committed range の bundle 化を最優先し、staged/unstaged/untracked は段階的に追加する
- Rationale:
  - 仕様既定のコマンド/契約に合わせ、後戻りを減らす
  - range指定 + determinism + split を先に固めると拡張が容易
- Implications:
  - 既存の `diffship tui`（ops可視化/loop支援）は維持し、handoffのTUI/previewは後続で扱う
---

## D-019: `diffship build` MVP は「committed-only + 1 part」から始め、出力を安定させる

- Date: 2026-03-05
- Decision:
  - `diffship build` の最初の実装は committed range のみ（staged/unstaged/untracked は後続）。
  - まずは `parts/part_01.patch` の 1part 固定で bundle レイアウト（`HANDOFF.md` + `parts/`）を確立する。
  - `--range-mode` は `direct|merge-base|last|root` を受け付け、デフォルトは `last`。
  - デフォルト出力は `./diffship_YYYY-MM-DD_HHMM/`（同名が存在する場合は失敗）。
  - `--zip` はディレクトリ出力と同じレイアウトで `.zip` を生成する。
- Rationale:
  - まず “AI に渡す入口（HANDOFF.md）と diff の置き場” を固定すると、split / attachments / preview を安全に積み増せる。
  - range の選択肢は先に揃えておくと、コミット単位 split 等へ繋げやすい。

---

## D-020: 長い render 関数の引数は Context struct にまとめて clippy を通す

- Date: 2026-03-05
- Decision:
  - `clippy::too-many-arguments` を避けるため、doc レンダリング等の引数が多い関数は `*Inputs` のような Context struct にまとめて渡す。
  - テストは標準ライブラリの `str::contains` で足りる場合、predicates の `Predicate` trait を持ち込まない。
- Rationale:
  - `-D warnings` 運用下で、実装都合の `#[allow(..)]` を増やさずに品質ゲートを維持するため。
- Implications:
  - 「引数が多い関数」は “構造化して渡す” をデフォルトにし、局所 allow は最後の手段にする。

---

## D-021: テストは default branch 名を仮定しない（main/master を自動検出）

- Date: 2026-03-05
- Decision:
  - 一時リポジトリを使う統合テストでは、`master` などの固定ブランチ名を前提にしない。
  - 必要な場合は `git rev-parse --abbrev-ref HEAD` で現在ブランチ名を取得し、それを checkout や CLI 引数に使う。
- Rationale:
  - Git の初期ブランチ名は環境設定で変わり得て、CI/ユーザー環境で `master` が存在しないケースがある。
- Implications:
  - テストが環境依存で落ちないようにし、`just ci` を安定させる。

---

## D-022: Handoff の uncommitted sources は segment toggle で段階導入する

- Date: 2026-03-06
- Decision:
  - `diffship build` は committed をデフォルト ON としつつ、`--include-staged` / `--include-unstaged` / `--include-untracked` で uncommitted sources を追加できるようにする。
  - committed を外したい場合は `--no-committed` を使う。
  - untracked はまず text add-diff のみを扱い、binary/unreadable は File Table に skip note を残す。raw attachments は M6-03 で導入する。
- Rationale:
  - まず AI に渡す差分の出どころ（segment）を明示できるようにし、後続の attachments/excluded/split を安全に積み増すため。
- Implications:
  - `HANDOFF.md` には各 segment の included 状態と base（HEAD / committed range）を明記する。

---

## D-023: Traceability の `Partial` は `TBD` が残る場合だけ使う

- Date: 2026-03-06
- Decision:
  - `docs/TRACEABILITY.md` で `Status: Partial` を使うのは、Tests か Code のどちらかに `TBD` が残る場合だけにする。
  - Tests と Code の両方が具体化されている項目は `Implemented` にする。
- Rationale:
  - `scripts/check-traceability.sh` の整合ルールに合わせ、`just trace-check` を安定して通すため。
- Implications:
  - 部分実装を表現したい場合でも、どちらか一方は `TBD` を残して `Partial` を使う。

---

## D-024: M6-03 の split / untracked 方針

- Date: 2026-03-06
- Decision:
  - `--split-by auto|file|commit` を導入し、`commit` は committed range にのみ適用する。
  - `auto` は committed range が複数コミットなら `commit`、それ以外は `file` に寄せる。
  - untracked は `--untracked-mode auto|patch|raw|meta` を持ち、`auto` は **text/small → patch / binary-or-unreadable-or-large → attachments.zip** とする。
  - `meta` のときは内容を同梱せず `excluded.md` に理由と再実行ガイダンスを残す。
- Rationale:
  - AI に読みやすい commit view を出しつつ、巨大/非UTF-8ファイルで handoff を壊さないため。
- Implications:
  - `HANDOFF.md` には Commit View / Attachments / Exclusions セクションを条件付きで出す。
  - staged / unstaged / untracked は file-level unit のままとする。

---

## D-025: docs-check 対象の README では生成物名を path backtick として書かない

- Date: 2026-03-06
- Decision:
  - `README.md` で **生成される出力物**（HANDOFF.md / parts/ / attachments.zip / excluded.md など）を inline code の path 参照として書かない。
  - それらは repo に存在するドキュメント/実装ファイルではないため、通常テキストとして記述する。
  - `zip::write::FileOptions` はこのリポジトリの依存版（0.6 系）に合わせ、型注釈は `FileOptions` をそのまま使う。
- Rationale:
  - `scripts/check-doc-links.sh` は README の backtick path を実在ファイルとして検証するため、生成物名を code path で書くと docs-check が落ちる。
  - 依存 crate の API 断面に合わせて型注釈を保守し、環境差分でビルドを壊さないため。
- Implications:
  - README では「実在する repo パス」と「実行後に生成される成果物」を書き分ける。
  - 依存 crate のメジャー更新時は `FileOptions` 周辺の型注釈を再確認する。

---

## D-026: HANDOFF.md は bundle の入口ドキュメントに固定する

- Date: 2026-03-06
- Decision:
  - `diffship build` が生成する HANDOFF.md は、bundle 全体の入口ドキュメントとして扱う。
  - 最低限 `Start Here` / `TL;DR` / `Change Map` / `Parts Index` を毎回含める。
  - `Parts Index` は quick index と part details の二段構成にし、読む順番を決めやすくする。
- Rationale:
  - AI や人間が bundle を開いたとき、最初に何を読めばよいか迷わないようにするため。
- Implications:
  - テストでは章立てと first patch の導線を確認し、出力の入口構造を壊さない。

---

## D-027: M6-05 の ignore / secrets warning 方針

- Date: 2026-03-06
- Decision:
  - build 側は `.diffshipignore` を直接読み、committed / staged / unstaged / untracked の各 segment に同じ除外ルールを適用する。
  - secrets-like content を検知した場合は `secrets.md` と `HANDOFF.md` に **path + reason only** で記録し、値は出さない。
  - 非TTYで secrets が出た場合は `--yes` がない限り exit code 4 で止める。CI では `--fail-on-secrets` を使う。
- Rationale:
  - handoff bundle をそのまま外部 AI に渡すことを想定すると、build 時点で共有リスクを見せる必要があるため。
  - ignore は source type ごとに挙動がズレると bundle の説明可能性が下がるため、全 segment で一貫適用にする。
- Implications:
  - `diffship build` は `--yes` / `--fail-on-secrets` を持つ。
  - HANDOFF.md は secrets warning / ignore active の状態を入口で明示する。

---

## D-028: 予約だけ先に入れる handoff exit code には `#[allow(dead_code)]` を付ける

- Date: 2026-03-06
- Decision:
  - handoff 側でも、SPEC 先行で exit code を予約する場合は実装が参照するまで `#[allow(dead_code)]` を付ける。
  - 今回の `EXIT_PACKING_LIMITS=3` は将来の size/profile 制御用として残し、未使用警告だけ抑制する。
- Rationale:
  - `clippy -D warnings` を壊さずに、SPEC と実装の番号対応を維持するため。
- Implications:
  - exit code の削除ではなく「予約したまま許容する」を基本にする。
  - 将来 M6-06 以降で packing limit 系の失敗を実装したら `allow` を外す。

---

## D-029: M6-06 では handoff 出力順序と zip metadata を固定し、golden tests を追加する

- Date: 2026-03-06
- Decision:
  - `HANDOFF.md` の File Table など、bundle内の一覧は **docs → config → source → tests → other** のカテゴリ順、その後 path 昇順、最後に segment 順（committed → staged → unstaged → untracked）で固定する。
  - diffship が生成する zip（bundle zip / attachments.zip）は、entry順をソートし、zip metadata の mtime は固定値（zip crate default）を使う。
  - determinism は `tests/m6_handoff_determinism.rs` と `tests/golden/` の fixture で守る。
- Rationale:
  - 同じ入力から同じ bundle tree / zip bytes を得られるようにして、golden tests と bundle比較を安定させるため。
- Implications:
  - 今後 ordering rule を変える場合は `docs/DETERMINISM.md` と golden fixture を同時更新する。
  - zip metadata を変える場合は raw zip 比較テストも見直す。

---

## D-030: golden 正規化は UTF-8 を保持する

- Date: 2026-03-06
- Decision:
  - golden fixture 比較用の正規化では、40桁hex置換をしても UTF-8 記号（例: `→`）を壊さない実装にする。
- Rationale:
  - byte 単位で文字列を再構成すると、非ASCII文字が文字化けして golden test が偽陽性で落ちるため。
- Implications:
  - placeholder 置換は char 境界で進める。
  - golden は「実出力差」だけを検出し、正規化起因の差分を混ぜない。
