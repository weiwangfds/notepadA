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

## Phase 3: 超大文件优化

### 3.1 后台索引构建 (Rust)
- [ ] 改造 `LineIndex::new()` — 只同步扫描前 2MB (已有 INITIAL_SCAN_BYTES)
- [ ] 添加 `LineIndex::build_background()` — 后台线程逐块扫描
- [ ] 使用 `Arc<AtomicU64>` 共享进度，通过 Tauri events 推送前端
- [ ] 添加 `get_line_count` 命令: 索引未完成时返回估计值
- [ ] 索引完成后发送 `index-complete` 事件

### 3.2 两级稀疏索引 (Rust)
- [ ] 改造 `LineIndex` 为两级结构 (主索引 + 块索引 LRU 缓存)
- [ ] 主索引: 每 4096 行一个条目 (内存常驻, ~2MB for 100GB)
- [ ] 块索引: LRU 缓存访问过的 4096 行块内详细偏移
- [ ] `line_offset()` 先查主索引再查块索引，未命中则按需构建块索引
- [ ] 单元测试: 验证两级索引的正确性和 LRU 淘汰

### 3.3 按需编码转换 (Rust)
- [ ] 改造 `ViewportManager` — 不再全量 UTF-8 转换
- [ ] 存储原始 mmap bytes + encoding info
- [ ] `get_viewport()` 时按需: 行索引定位 → 读原始字节 → 转换 UTF-8
- [ ] 添加 UTF-8 转换结果的 LRU 缓存
- [ ] 性能测试: 验证大文件打开不再 OOM

### 3.4 前端优化
- [ ] 改进自定义滚动条: 支持精确模式 (索引完成后) 和估算模式
- [ ] 实现 Goto Line 对话框组件 (替代 `prompt()`)
- [ ] 滚动条点击跳转 (点击轨道任意位置)
- [ ] 滚动条拖拽时显示行号提示
- [ ] 键盘 Page Up/Page Down 支持

### 3.5 测试验证
- [ ] 编写脚本生成 1GB+ 测试文件
- [ ] 验证打开大文件内存占用 < 50MB
- [ ] 验证滚动流畅度 (< 30ms 一屏)
- [ ] 验证跳转到文件头/尾响应时间

---

## Phase 4: 搜索与替换

### 4.1 文本搜索引擎 (Rust)
- [ ] 创建 `src-tauri/src/search/mod.rs`
- [ ] 实现 `text_search.rs` — Boyer-Moore 字符串搜索
- [ ] 实现流式搜索: 逐块扫描，通过 channel 返回结果
- [ ] 支持大小写敏感/不敏感选项
- [ ] 支持全字匹配选项

### 4.2 正则搜索引擎 (Rust)
- [ ] 实现 `regex_search.rs` — 基于 `regex` crate
- [ ] 支持跨行匹配选项
- [ ] 正则搜索也使用流式返回

### 4.3 搜索命令 (Rust)
- [ ] 创建 `src-tauri/src/commands/search_cmds.rs`
- [ ] 实现 `search(tab_id, query, options)` — 启动搜索，返回 search_id
- [ ] 实现 `search_next(tab_id, search_id)` — 获取下一个匹配
- [ ] 实现 `replace_all(tab_id, search_id, replacement)` — 全部替换
- [ ] 在 `lib.rs` 注册搜索命令

### 4.4 搜索 UI (前端)
- [ ] 创建 `SearchBar.tsx` 组件 (Ctrl+F 触发)
- [ ] 创建 `ReplaceBar.tsx` 组件 (Ctrl+H 触发)
- [ ] 搜索结果高亮渲染
- [ ] 上一个/下一个匹配导航 (F3/Shift+F3)
- [ ] 搜索结果计数显示
- [ ] 替换当前/全部替换按钮

---

## Phase 5: 完善体验

### 5.1 多标签增强
- [ ] 标签拖拽排序
- [ ] 标签右键菜单 (关闭其他、关闭右侧)
- [ ] 未保存标签关闭时确认对话框

### 5.2 拖放支持
- [ ] 支持拖放文件到编辑器打开
- [ ] 支持命令行参数打开文件

### 5.3 主题与外观
- [ ] 实现深色/浅色主题切换 (MenuBar 添加切换按钮)
- [ ] HeroUI 集成: 替换自定义 CSS 为 HeroUI 组件
- [ ] 编辑器字体/字号设置

### 5.4 最近文件
- [ ] 记录最近打开的文件列表
- [ ] 菜单显示最近文件
- [ ] 持久化到本地配置文件

### 5.5 可选增强
- [ ] 基础语法高亮 (基于 Tree-sitter 或简单正则)
- [ ] 右侧 Minimap 缩略图
- [ ] 性能监控面板 (开发调试用)
- [ ] 行号点击选中整行

---

## 开发顺序

当前进度: **Phase 3 超大文件优化** ← 下一步从这里开始

1. ✅ Phase 1 — 已完成
2. ✅ Phase 2 — 已完成 (Piece Table + 编辑 + 保存)
3. 🔄 Phase 3 — 大文件优化 (当前任务)
4. ⬜ Phase 4 — 搜索替换
5. ⬜ Phase 5 — 完善体验
