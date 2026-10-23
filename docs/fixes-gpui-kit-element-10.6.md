# gpui-kit 上游缺陷修复报告 — gpui-kit-element 10.6

- 仓库：gpui-kit（上游），修复由 gpui-kit-element「add-native-component-gallery」战役 10.6 条登记的 5 项上游缺陷。
- 基线提交：`4b799fc7f66b16840778e327ef3b74275a347ee1`（修复所在提交以最终 `git rev-parse HEAD` 为准）。
- 纪律：每项 TDD（先失败测试后修复）；gpui-pre 0.3.7 为 crates.io 精确 pin（=0.3.7），**不可修改**——根因在 gpui-pre 层的缺陷在 gpui-kit component/base 层实现防御/包装并文档化上游限制。
- 本仓已有未提交改动（`crates/component/src/form/tests.rs` 格式化差异）非本报告产物，保持不动；两仓均不执行 git 提交。

## 缺陷清单与状态

| # | 缺陷 | 根因层 | 状态 |
|---|------|--------|------|
| 1 | F1 `subscribe_in` 双租约 panic（entity_map.rs:164） | gpui-pre（不可改），gpui-kit 侧防御 | 已修复（SafeUpdateExt，2/2 绿） |
| 2 | `flush_effects` 确定性重绘不收敛 | gpui-pre（不可改），守卫下沉到 setter | 已修复（set_placeholder 变更守卫，1/1 绿） |
| 3 | 共享交互状态多实例合并 | gpui 语义限制，组件层显式 API + 文档化 | 已修复（instance_id API，3/3 绿） |
| 4 | ScrollHandle 每帧重建致滚动不保持 | gpui-kit 组件层 | 已修复（restore_scroll_handle，2/2 绿） |
| 5 | RadioGroup 子项 id 覆写为整数索引（可观察性） | gpui-kit 组件层 | 已修复（保留显式 id，3/3 绿） |

## 1. F1 subscribe_in 双租约 panic

（逐项追加）

## 2. flush_effects 确定性重绘不收敛

（待追加）

## 3. 共享交互状态多实例合并

（待追加）

## 4. ScrollHandle 每帧重建

（待追加）

## 5. RadioGroup 子项 id 整数索引覆写

- **现象**（element 侧实录：agent-batch-dialog.md「form 三处实测修正」/agent-batch-table.md）：
  `RadioGroup::render`（`crates/component/src/radio.rs`）将每个子项 id 覆写为
  `radio.id = ix.into()`，应用方设置的 `"al-pos-top"` / `"sc-size-large"` 等字符串 id
  不可寻（观察注册表 UnknownTarget），验收侧被迫改用「组锚点 + 索引」定位。
- **根因**：组件层覆写，非 gpui-pre 层。索引 id 还使不同 RadioGroup 的同位置子项
  在观察注册表全局歧义（`Integer(0)` 多处命中）。
- **修复**（TDD：`crates/component/tests/radio_group_ids.rs` 先 3 红）：
  保留子项显式 id；仅当同组内出现重复 id（同帧双重身份会导致 element state 合并、
  交互失效）时，该重复项回退为历史索引 id `Integer(ix)`。
  位置语义不变：`on_click(&ix)`、`position_in_set`/`size_of_set` 照旧。
- **验证**：`cargo test -p gpui-component --features test-support --locked --test radio_group_ids`
  3/3 通过；组件库全量 `cargo test -p gpui-component --features test-support --locked`
  全绿（含 579 lib 测试）。
- **注意**：涉及观察注册表的测试需以 `--features test-support` 运行（`Observed`
  包装类型的配套 impl 在 `scrollable.rs:292` 等处以组件自身 feature gate）。
- **升 pin 影响**：element 仓 e2e 若升级 pin，原先「组内索引定位」的 Radio 用例需
  回归核验——正常用例（子项 id 唯一）现在可直接按原名寻址；历史「重复 id 回退索引」
  行为与旧版一致，无需改测试（其按组作用域定位仍成立）。

## 4. ScrollHandle 每帧重建致滚动不保持

- **现象**（element 侧 10.6 登记根因）：`v_virtual_list`/`h_virtual_list`
  （`crates/base/src/virtual_list.rs` 的 `virtual_list()` 构造器）每次构造都
  `VirtualListScrollHandle::new()`。所有者在 render 内重建列表（虚拟表、
  infinite-scroll 场景）时，每帧得到全新句柄 → 滚动偏移回零，滚轮滚动"不吸住"。
- **根因层**：gpui-kit 组件层（gpui-pre 的 `ScrollHandle` 偏移存于句柄自身 Rc；
  未带句柄的 overflow_scroll 容器才会把偏移持久化在 element state）。
- **修复**（TDD：`crates/base/tests/virtual_list_scroll.rs` 先 1 红 1 绿）：
  `VirtualList::request_layout` 开头新增 `restore_scroll_handle`——把上一帧的
  实际句柄存入本元素的 window element state；若本帧新句柄与存档句柄 Rc 身份
  不同（所有者重建了句柄），则把存档句柄上的**活偏移**（含帧间滚轮增量）拷贝进
  新句柄，并重新存档新句柄。持久句柄（如 table 路径）按身份匹配，零拷贝零影响。
- **验证**：`cargo test -p gpui-base --features test-support --locked --test virtual_list_scroll`
  2/2（重建句柄场景 + 持久句柄回归守卫）；gpui-base 全量 1226 lib 测试 + 3 测试文件全绿。
- **升 pin 影响**：行为向前改善（滚动保持），无 API 变化；element 仓
  virtual-list/infinite-scroll 相关验收升级 pin 后预期由"失败/绕行"转绿，需复跑核验。

## 3. 共享交互状态多实例合并（input-number 多实例按钮失效根因）

- **实证复现**（`crates/component/tests/number_input_shared_state.rs`，TDD 先红）：
  官方 shared v-model demo（同 `InputState` 驱动多个 NumberInput、祖先无 id）下，
  全部实例以**同一 GPUI 元素身份**绘制：`[("number-input", state_id), "increment"]`。
  实测：观察注册表仅保留最后绘制实例（HashMap 单槽覆盖）；点击含帧间重绘的
  down→draw→up 序列时后绘制面板按钮 on_click 不触发（值恒 "1"）——与
  agent-batch-tree.md 实录一致。单实例/独立 state 双实例不受影响（对照实验）。
- **两层问题、两层处置**：
  1. **同 ElementId 身份合并（本缺陷登记的根因，组件层已修）**：新增显式、稳定的
     `NumberInput::instance_id(impl Into<ElementId>)`（base + component 双层 API）。
     设置后 content 子树以调用方 id 命名空间化 → 各实例元素身份唯一，注册表与
     element state 隔离；`increment`/`decrement` 叶子 id 与 `("number-input", entity)`
     frame id 保持不变（观察作用域兼容）。实测：命名后双实例含重绘序列均可激活，
     共享值 1→2→3。
  2. **残余上游限制（文档化，不可组件层根治）**：不设置 instance_id 时，合并行为
     与旧版完全一致（`unnamed_shared_instances_stay_collapsed` 契约测试钉住）。
     gpui 语义上"同帧同身份多实例"本就是应用层错误；稳定的自动身份推导不可行
     ——实验证伪了两个方案：帧序号计数器（gpui-pre 无帧号暴露，用 `App::defer`
     归零会在帧中段 effect flush 时漂移：同帧重绘中两个实例拿到同一序号，
     恶化既有行为，已废弃该实现）；复合 frame id（引入了独立缺陷的左面板回归，
     已回退）。
- **验证**：`cargo test -p gpui-component --features test-support --locked
  --test number_input_shared_state` 3/3；component 全量（579 lib + 9 测试文件）、
  base 全量（1226 lib）全绿。
- **升 pin 影响**：默认行为逐字节不变（无 instance_id 时零改动）；element 仓升级
  pin 后，共享面板若要真实点击需在 story 中加 `.instance_id(...)`，否则继续用
  已验证的键盘语义（Increment/Decrement action，单实例按钮路径不变）。

## 1. F1 subscribe_in 双租约 panic

- **现象**（element 侧实录 agent-batch-cascader.md F1）：`subscribe_in` 处理器内经
  捕获的自身句柄调用 `Entity::update` → gpui-pre `invoke_subscriber_in` 已在
  `subscriber.update(...)` 内租借订阅者，二次租约在 `entity_map.rs:164`
  `lease_erased` panic：`cannot update ... while it is already being updated`
  （date-formats/default-time demo 选日即崩，结构性与窗口无关）。
- **根因层**：gpui-pre（不可改）。`Context::entity_id()`/`Context::entity()` 为
  pub API，处理器已持有同实体的 `&mut T`，可完全绕开二次租约。
- **修复**（TDD：`crates/base/tests/safe_update.rs` 先 2 红）：
  新增 `crates/base/src/safe_update.rs` —— `SafeUpdateExt::update_reentrant`：
  目标实体 == 当前上下文实体（已被租借）时，对处理器自带的 `&mut T` 就地执行
  闭包；其他实体走常规 `Entity::update`。两条路径 `f` 均获得 `&mut T` +
  `Context<T>`，`cx.notify()` 语义一致。
  测试：①原始模式 `#[should_panic(expected = "while it is already being updated")]`
  精确复现双租约；②`update_reentrant` 同场景恰好执行一次且 notify 生效。
  测试基建要点：`subscribe_in` 返回的 `Subscription` 必须持有（丢弃即注销）。
- **验证**：`cargo test -p gpui-base --features test-support --locked --test safe_update`
  2/2 通过；base 全量回归绿。
- **下游建议**：element 仓 date-formats/default-time story 订阅闭包改用
  `cx.update_reentrant(this, &view, ...)`（或直接用处理器 `&mut Story` + `cx.notify()`），
  即可收窄覆盖回到全语义。

## 2. flush_effects 确定性重绘不收敛

- **现象**（element 侧实录 agent-pagination.md F1 + element 仓 923b3200 提交）：
  渲染期 `window.defer` 无条件写入 `set_placeholder`（其无条件 `notify`）造成
  「render → defer → notify → dirty → redraw」；gpui-pre 0.3.7 确定性测试调度
  （`GpuiMode::Test` 的 `flush_effects` 内联重绘 dirty 窗口直至收敛）下永不收敛，
  单次 `render_frame` 挂死。slider-sizes 共享态 + thumb 悬停收尾的同类挂起同属
  「每帧 notify 不收敛」家族。
- **根因层**：gpui-pre 的确定性重绘调度不可改；组件侧可根治的是"渲染期无条件
  notify 写入"。element 在其 ElInput 调用侧加了变更守卫；上游通用解法是把守卫
  **下沉到 `InputState::set_placeholder` setter 本身**——保护所有调用方（含上游
  `list.rs:640` 这个未守卫的渲染期写入点）。
- **修复**（TDD：`crates/base/tests/input_placeholder.rs` 先红——同值写入 ×3
  通知数 2→5）：`crates/base/src/input/base/state.rs` 的 `set_placeholder` 在
  占位文案未变化时跳过写入与 notify；真实变化照常通知。
  上游既有守卫调用点（`command/state.rs` 的 `applied_placeholder`、
  component-shell `input_group/binding.rs`）不受影响；`list.rs` 渲染期写入点
  由 setter 守卫自动免疫。
- **验证**：`cargo test -p gpui-base --features test-support --locked --test
  input_placeholder` 1/1；base 1226 lib 全量、component 全量回归绿。
- **诚实边界**：slider-sizes 的「共享态 + thumb 悬停收尾」挂起链路（涉及悬停态
  与共享实体组合的具体不收敛路径，gpui-pre 内部）未完全确证；本修复根治的是
  同族的"渲染期无条件 notify 写入"通用模式，并在报告中登记该已知限制。
  治本需 gpui-pre 的 flush_effects 收敛上限/dirty 去重（上游 snapshot 更新时跟进）。
- **升 pin 影响**：行为收紧（同值写入不再产生通知/重绘），无 API 变化；依赖
  "同值写入必 notify" 的用例（若有）需回归核验——按官方语义不应存在。

（全部缺陷已修复，以下为总验证与升 pin 评估，待追加）

## 验证与升 pin 建议

### 变更清单（相对基线 4b799fc7，未提交）

| 文件 | 变更 |
|---|---|
| `crates/base/src/safe_update.rs` | 新增：`SafeUpdateExt::update_reentrant`（缺陷①） |
| `crates/base/src/lib.rs` | 注册 `safe_update` 模块与导出 |
| `crates/base/src/input/base/state.rs` | `set_placeholder` 变更守卫（缺陷②） |
| `crates/base/src/number_input.rs` | base `NumberInput::instance_id` API + content 命名空间（缺陷③） |
| `crates/component/src/input/number_input.rs` | component `NumberInput::instance_id` 透传（缺陷③） |
| `crates/base/src/virtual_list.rs` | `restore_scroll_handle`（缺陷④） |
| `crates/component/src/radio.rs` | RadioGroup 保留子项显式 id、重复回退索引（缺陷⑤） |
| `crates/base/tests/{safe_update,input_placeholder,virtual_list_scroll}.rs` | 新增测试（①②④） |
| `crates/component/tests/{number_input_shared_state,radio_group_ids}.rs` | 新增测试（③⑤） |
| `crates/component/src/form/tests.rs` | **非本报告产物**（先前已存在的格式化差异，未触碰） |

### 测试证据（Windows 11 x64，cargo 1.99.0，--locked）

```text
cargo test -p gpui-base    --features test-support --locked
# 1226 lib + 1(element_ext) + 1(input_placeholder②) + 2(safe_update①) + 2(virtual_list_scroll④) 全绿
cargo test -p gpui-component --features test-support --locked
# 579 lib + 9 测试文件全绿，含 radio_group_ids⑤ 3/3、number_input_shared_state③ 3/3
cargo test -p gpui-kit --features "test-support component" --locked
# 22 个测试目标 278 passed / 0 failed（含 controls/input/dock 等全部 kit 集成测试）
```

每项均先跑红后修绿（红证据见各节）。

### 修复所在提交

基线与工作区一致：`4b799fc7f66b16840778e327ef3b74275a347ee1`（未产生新提交；
两仓均按纪律未 git commit）。升 pin 时以上述变更清单打包为一个上游提交即可。

### 升 pin 建议与影响评估

- **不强制升 pin**：element 仓 `rev=5d1fcc92` 当前验收已闭环（绕行/收窄策略全部
  绿）。本次修复为行为改善 + 新 API，无破坏性变更，升级可择机进行。
- **建议升 pin**（一次性打包上述变更），预期收益：
  1. 10.6-①：date-formats/default-time 的 Value 镜像回写可用 `update_reentrant`
     恢复全语义（element 侧 story 需同步改用新 API 或 `&mut T` + notify）；
  2. 10.6-②：placeholder 每帧 notify 挂死根源在 setter 层消除，element 侧
     923b3200 的调用侧守卫可保留（双保险）；
  3. 10.6-④：virtual-list/infinite-scroll 滚动保持，相关验收预期由绕行转绿；
  4. 10.6-⑤：RadioGroup 原名可寻，"组锚点+索引"定位仍兼容（唯一 id 保留原名，
     重复 id 回退索引与旧版一致）。
- **升 pin 风险点（需回归核验）**：
  1. `set_placeholder` 同值写入不再通知——若 element 侧有依赖"同值写入触发
     notify"的用例（按官方语义不应存在），需调整；
  2. input-number 共享态面板：默认渲染行为逐字节不变；若要真实点击需在 story
     加 `.instance_id(...)`，否则继续使用已验证的键盘语义（Increment/Decrement
     action）——涉及 acceptance 的 `native_input_number_story_e2e.rs` 无需改动
     （其共享面板本就走键盘路径，单实例面板按钮路径不变）；
  3. RadioGroup 子项 id 变化：element e2e 中按组内**整数索引**定位的 Radio 用例
     （`al-pos`、`sc-size`、date-range-size 等）——子项 id 唯一时叶子 id 变为原
     字符串名，`within(组).click(索引)` 需改为按名寻址或保留组内索引语义的适配；
     重复 id 场景回退行为不变。
- **升 pin 操作面**：element 仓 `Cargo.toml` 的 git rev 指向新提交 + `cargo update
  -p gpui-kit*`；影响面 8 个 gpui-kit* 包，建议在 element 仓空闲窗口执行
  `RUST_TEST_THREADS=4 cargo test -p gpui-kit-element-acceptance --locked` 全量回归。

### 遗留与边界

- gpui-pre 0.3.7 内部机制未改（crates.io pin 不可改）：flush_effects 收敛上限/
  dirty 去重、同帧同 ElementId 多实例合并语义、`invoke_subscriber_in` 租约复用——
  三项均以 gpui-kit 侧防御 + 文档化交付，治本需跟进 gpui-pre snapshot 升级。
- slider-sizes「共享态 + thumb 悬停收尾」挂起的具体链路未完全确证（element 侧
  8 轮探针矩阵定位到触发条件与逃逸解法）；本批根治同族的渲染期无条件 notify 模式。
- 平台矩阵：Windows（cargo test 全量实测）；macOS/Linux 未验证。
