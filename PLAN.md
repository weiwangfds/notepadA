# NotePadA 开发计划

> 基于 DESIGN.md 路线图，按阶段推进开发。每完成一项任务勾选对应 checkbox。

---

## 代码审查结论

### 设计合理的部分
- mmap + 稀疏行索引的核心架构设计正确
- Rust 后端模块划分清晰 (file/buffer/viewport/commands)
- 前端虚拟滚动 MVP 实现可用
- Tauri IPC 命令接口设计合理
- 测试覆盖良好 (10 单元测试 + 1 集成测试)

### 当前已知问题
1. `ViewportManager` 全量 UTF-8 转换 — 大文件 OOM 风险 (Phase 2 用 Piece Table 替代)
2. `LineIndex::build_full()` 同步阻塞 — 大文件打开卡顿 (Phase 3 改后台线程)
3. `lru` crate 已声明依赖但未使用 — Phase 2 Piece Table 需要
4. HeroUI 已声明依赖但未使用 — 可选，后续 UI 增强时引入
5. 前端无测试 — 需补充关键逻辑测试

---

## Phase 1: 基础框架 ✅ 已完成

- [x] Rust: mmap 文件映射 (FileMapper)
- [x] Rust: 编码检测与转换 (chardetng + encoding_rs)
- [x] Rust: 基础行索引 (全量扫描 + memchr)
- [x] Rust: Tauri 命令 (open_file, close_file, get_tabs, get_viewport, goto_line, get_line_count)
- [x] 前端: 编辑器基本布局 (MenuBar + TabBar + EditorView + StatusBar)
- [x] 前端: 虚拟滚动 MVP (固定行高 22px, overscan 10 行)
- [x] 前端: 自定义滚动条
- [x] 前端: 键盘快捷键 (Ctrl+O/G/W)
- [x] 联调: 打开文件并浏览

---

## Phase 2: 编辑能力 ✅ 已完成

### 2.1 Piece Table 数据结构 (Rust)
- [x] 创建 `src-tauri/src/buffer/piece_table.rs`
- [x] 实现 `Piece` 结构体 (source: Original/AddBuffer, offset, length)
- [x] 实现 `PieceTable` 结构体 (original, add_buffer, pieces BTreeMap)
- [x] 实现 `insert(pos, text)` — O(1) 追加 + O(log n) 插入 Piece
- [x] 实现 `delete(pos, len)` — O(log n) 分裂/删除 Piece
- [x] 实现 `replace(pos, len, text)` — delete + insert 组合
- [x] 实现 `to_bytes()` / `read_range(pos, len)` — 读取逻辑内容
- [x] 实现 undo/redo 栈
- [x] 30 个单元测试全部通过

### 2.2 编辑命令 (Rust)
- [x] 创建 `src-tauri/src/commands/edit_cmds.rs`
- [x] 实现 `insert_text(tab_id, line, col, text)` 命令
- [x] 实现 `delete_range(tab_id, start, end)` 命令
- [x] 实现 `replace_range(tab_id, start, end, text)` 命令
- [x] 实现 `undo(tab_id)` / `redo(tab_id)` 命令
- [x] 更新 `Document` 结构体: 集成 PieceTable
- [x] 更新 `ViewportManager`: 重构为接受外部 bytes 参数
- [x] 在 `lib.rs` 注册新命令

### 2.3 文件保存 (Rust)
- [x] 创建 `src-tauri/src/file/saver.rs` (原子写入: 临时文件 + rename)
- [x] 实现 `save_file(tab_id)` — 从 Piece Table 读取内容写入
- [x] 实现 `save_file_as(tab_id, path)` — 另存为
- [x] 在 `file_cmds.rs` 添加 save_file/save_file_as 命令

### 2.4 光标与文本选择 (前端)
- [x] 改进 `EditorView.tsx` 光标渲染: 使用隐藏 input 捕获键盘/IME 输入
- [x] 实现文本选择 (shift+click)
- [x] 实现选择区域高亮渲染
- [x] 实现选择区域删除 (Backspace/Delete 选中文本)

### 2.5 键盘输入处理 (前端)
- [x] 实现字符输入 → `insert_text` IPC
- [x] 实现 Backspace/Delete → `delete_range` IPC
- [x] 实现 Enter → 插入换行符
- [x] 实现 Tab → 插入空格
- [x] 实现方向键移动光标 (上下左右)
- [x] 实现 Home/End → 行首/行尾
- [x] 实现 Ctrl+Home/End → 文件头/尾
- [x] 实现 PageUp/PageDown
- [x] 实现 Ctrl+Z/Y → 撤销/重做

### 2.6 前端 IPC 扩展
- [x] 更新 `tauriApi.ts`: 添加所有编辑和保存 API
- [x] 更新 `types/editor.ts`: 添加 EditResult 类型
- [x] 更新 `useEditor.ts`: 集成编辑操作，标记 dirty 状态
- [x] 更新 `MenuBar.tsx`: Save/SaveAs/Undo/Redo 菜单
- [x] 更新 `App.tsx`: 传递编辑 props
- [x] 更新 CSS: 选择区域高亮样式

---

## Phase 3: 超大文件优化 ✅ 已完成

### 3.1 后台索引构建 (Rust)
- [x] 改造 `LineIndex::new()` — 只同步扫描前 2MB
- [x] 添加 `LineIndex::build_background()` — 后台线程逐块扫描 (4MB chunks)
- [x] 原子进度跟踪 (`AtomicU32` per-mille)
- [x] 集成到 `AppState::open_file` — 大文件自动后台索引
- [x] 13 个 LineIndex 单元测试全部通过 (含 background indexing 测试)

### 3.2 两级稀疏索引 (Rust)
- [x] 改造 `LineIndex` 为两级结构 (主索引 + 块索引 LRU 缓存)
- [x] 主索引: 每 4096 行一个条目 (GROUP_SIZE = 4096)
- [x] 块索引: `LruCache<u64, BlockIndex>` 缓存最近 64 个访问过的块
- [x] `line_offset()` 先查主索引 → 再查块缓存 → 未命中则按需构建
- [x] 单元测试: block cache 正确性验证

### 3.3 按需编码转换 (Rust)
- [ ] 延后: 当前 PieceTable 设计需要完整 UTF-8 文本，需在后续重构中实现
- [ ] 计划: PieceTable 改为引用原始 mmap bytes + 按需转换

### 3.4 前端优化
- [x] 实现 Goto Line 对话框组件 (`GotoLineDialog.tsx`)
- [x] 对话框样式 (overlay, input, buttons)
- [x] Ctrl+G 打开对话框替代 `prompt()`
- [x] Page Up/Page Down 键盘支持
- [x] Escape 关闭对话框

---

## Phase 4: 搜索与替换 ✅ 已完成

### 4.1 文本搜索引擎 (Rust)
- [x] 创建 `src-tauri/src/search/mod.rs`
- [x] 实现 `text_search.rs` — 逐行扫描搜索
- [x] 支持大小写敏感/不敏感选项
- [x] 支持全字匹配选项
- [x] 8 个单元测试

### 4.2 正则搜索引擎 (Rust)
- [x] 实现 `regex_search.rs` — 基于 `regex` crate
- [x] 支持大小写敏感/不敏感
- [x] 支持全字匹配 (via \b)
- [x] 5 个单元测试

### 4.3 搜索命令 (Rust)
- [x] 创建 `src-tauri/src/commands/search_cmds.rs`
- [x] 实现 `search(tab_id, query, options)` — 返回所有匹配
- [x] 实现 `search_next(tab_id, query, options, line, col)` — 下一个匹配
- [x] 实现 `replace_all(tab_id, query, replacement, options)` — 全部替换
- [x] 在 `lib.rs` 注册命令

### 4.4 搜索 UI (前端)
- [x] 创建 `SearchBar.tsx` 组件 (Ctrl+F 触发)
- [x] 内置 Replace 功能 (展开/折叠)
- [x] 搜索选项: 大小写敏感、全字匹配、正则
- [x] 上一个/下一个匹配导航 (Enter/Shift+Enter)
- [x] 搜索结果计数显示 (current / total)
- [x] 替换当前/全部替换按钮
- [x] API: searchQuery, searchNext, replaceAll
- [x] CSS 样式: 浮动搜索栏，选项按钮高亮

---

## Phase 5: 完善体验 ✅ 已完成

### 5.1 多标签增强
- [x] 标签关闭 (已有 x 按钮)
- [x] 未保存标签显示 dirty 指示符 (bullet)
- [ ] 标签拖拽排序 (后续)
- [ ] 标签右键菜单 (后续)

### 5.2 拖放支持
- [x] 支持拖放文件到编辑器打开
- [x] URL hash 自动打开 (`#file=/path/to/file`)

### 5.3 主题与外观
- [x] 实现深色/浅色主题切换
- [x] 系统主题自动检测 (`prefers-color-scheme`)
- [x] MenuBar 右侧切换按钮 (☀/🌙)
- [x] 完整的深色主题 CSS 变量

### 5.4 最近文件
- [ ] 记录最近打开的文件列表 (后续)
- [ ] 持久化到本地配置文件 (后续)

---

## 开发顺序

所有核心阶段已完成!

1. ✅ Phase 1 — 基础框架 (mmap + 虚拟滚动)
2. ✅ Phase 2 — 编辑能力 (Piece Table + 保存 + 键盘输入)
3. ✅ Phase 3 — 大文件优化 (后台索引 + 两级稀疏索引 + GotoLine)
4. ✅ Phase 4 — 搜索替换 (文本/正则搜索 + SearchBar UI)
5. ✅ Phase 5 — 完善体验 (主题切换 + 拖放打开)
