# NotePadA 技术架构设计

> 轻量级文本编辑器 — 低内存、低 CPU、支持 10GB~100GB 超大文件

---

## 1. 核心设计原则

| 原则 | 说明 |
|------|------|
| **内存受限** | 任何时刻内存占用与文件大小无关，只与可视区域大小有关 |
| **延迟加载** | 只加载用户正在查看的内容，其余内容按需加载 |
| **Rust 做重活** | 文件 I/O、索引构建、搜索、编辑数据结构全部在 Rust 侧完成 |
| **前端极轻** | 前端只负责渲染可视区域，不持有文件内容 |
| **渐进式就绪** | 打开文件即可浏览，索引在后台逐步构建 |

---

## 2. 总体架构

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (React)                      │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  Virtual     │  │  Custom      │  │  Menu /       │  │
│  │  Scroller    │──│  TextRenderer│  │  StatusBar    │  │
│  │  (viewport)  │  │  (DOM/Canvas)│  │               │  │
│  └──────┬───────┘  └──────┬───────┘  └───────────────┘  │
│         │                 │                             │
│         └────────┬────────┘                             │
│                  │  Tauri IPC (commands + events)       │
└──────────────────┼──────────────────────────────────────┘
                   │
┌──────────────────┼──────────────────────────────────────┐
│                  ▼       Backend (Rust)                  │
│  ┌───────────────────────────────────────────────────┐  │
│  │              Document Manager                      │  │
│  │  ┌────────────┐ ┌─────────────┐ ┌──────────────┐ │  │
│  │  │  Piece     │ │  Line Index │ │  Viewport    │ │  │
│  │  │  Table     │ │  (B-Tree)   │ │  Manager     │ │  │
│  │  └────────────┘ └─────────────┘ └──────────────┘ │  │
│  │  ┌────────────┐ ┌─────────────┐ ┌──────────────┐ │  │
│  │  │  File      │ │  Search     │ │  Encoding    │ │  │
│  │  │  Mapper    │ │  Engine     │ │  Detector    │ │  │
│  │  └────────────┘ └─────────────┘ └──────────────┘ │  │
│  └───────────────────────────────────────────────────┘  │
│                  │                                      │
│                  ▼                                      │
│         ┌────────────────┐                              │
│         │  OS mmap / I/O │  ← 不将整个文件载入内存      │
│         └────────────────┘                              │
└─────────────────────────────────────────────────────────┘
```

---

## 3. 超大文件核心机制

### 3.1 内存映射 (mmap) — 文件不进用户态内存

```
┌─────────────────────────────────────────┐
│  100GB File on Disk                      │
│  ┌──────┬──────┬──────┬──────┬─────     │
│  │Page 0│Page 1│Page 2│Page 3│...       │
│  │ 4KB  │ 4KB  │ 4KB  │ 4KB  │          │
│  └──┬───┴──────┴──┬───┴──────┴─────     │
│     │             │                      │
│     ▼             ▼                      │
│  OS Page Cache (自动管理)                │
│     │             │                      │
│     ▼             ▼                      │
│  Rust 通过 mmap 按需访问                 │
│  只映射当前需要的页面，内存占用 ≈ viewport │
└─────────────────────────────────────────┘
```

- 使用 `memmap2` crate，将文件映射到虚拟地址空间
- OS 自动管理页面换入换出，进程实际内存占用极小
- 读操作零拷贝，直接从 page cache 读取

### 3.2 行索引 (Line Index) — 快速定位行号

对于 100GB 文件可能有数十亿行，不可能为每行存储偏移量。

**两级稀疏索引结构：**

```
Level 1: 主索引 (Resident in memory)
┌─────────────────────────────────────────────┐
│  每 4096 行记录一个偏移量                      │
│  [0: offset_0, 4096: offset_4096, ...]      │
│  100GB ≈ 10亿行 → 主索引 ≈ 24万条 ≈ 2MB      │
└─────────────────────────────────────────────┘

Level 2: 块索引 (On-demand, 可持久化到临时文件)
┌─────────────────────────────────────────────┐
│  每个 4096 行块内部的详细行偏移                │
│  仅对用户浏览过的区域构建                      │
│  按需加载/卸载，内存占用可控                   │
└─────────────────────────────────────────────┘
```

**索引构建策略：**

1. **即时响应**：文件打开时，同步扫描前 N 行（如前 1MB），立即返回给前端显示
2. **后台构建**：启动后台线程逐步扫描整个文件，构建主索引
3. **增量可用**：索引构建过程中，已构建部分立即可用于跳转
4. **持久化**：索引保存到临时文件，再次打开同名文件时可复用

```rust
/// 行索引核心结构
struct LineIndex {
    /// 主索引: 每 GROUP_SIZE 行记录一次偏移
    /// key = line_number / GROUP_SIZE, value = file_offset
    sparse_offsets: Vec<u64>,
    /// 块索引缓存: LRU 缓存最近访问的块
    block_cache: LruCache<u64, BlockIndex>,
    /// 总行数 (后台构建完成后赋值)
    total_lines: Option<u64>,
    /// 文件总大小
    file_size: u64,
}

struct BlockIndex {
    /// 块内每行的起始偏移
    line_offsets: Vec<u32>,
    /// 块起始行的行号
    start_line: u64,
}
```

### 3.3 Piece Table — 高效编辑数据结构

Piece Table 是 VS Code 和很多现代编辑器使用的编辑数据结构，天然适合大文件：

```
原始文件 (只读，mmap 引用，永远不修改)
┌──────────────────────────────────────────┐
│ Line 1 content...\nLine 2 content...\n   │
│ ...原始文件内容保持不变...                  │
└──────────────────────────────────────────┘

追加缓冲区 (Append Buffer, 仅追加)
┌──────────────────────────────────────────┐
│ 用户新输入的文本追加到这里                    │
└──────────────────────────────────────────┘

Piece 描述符表 (描述文档逻辑顺序)
┌──────────────────────────────────────────┐
│ Piece 1: [Original, offset=0,    len=50] │
│ Piece 2: [AddBuf,   offset=0,    len=10] │  ← 用户新插入的内容
│ Piece 3: [Original, offset=50,   len=100]│
│ Piece 4: [AddBuf,   offset=10,   len=5]  │  ← 另一次插入
│ Piece 5: [Original, offset=150,  len=∞]  │
└──────────────────────────────────────────┘
```

**关键特性：**
- 插入操作：O(1) 追加到 Add Buffer + O(log n) 在 Piece Table 中插入描述符
- 删除操作：O(log n) 分裂/修改 Piece 描述符，不触碰原始数据
- 原始文件始终通过 mmap 引用，不产生内存拷贝
- Piece Table 使用 B-Tree 或平衡树组织，支持高效的范围查询

```rust
/// Piece Table 核心结构
struct PieceTable {
    /// 原始文件 mmap 引用 (只读)
    original: Mmap,
    /// 追加缓冲区
    add_buffer: Vec<u8>,
    /// Piece 描述符，用 B-Tree 按逻辑偏移组织
    pieces: BTreeMap<u64, Piece>,
    /// 行索引缓存 (基于 Piece Table 的逻辑行号)
    line_cache: LineIndex,
}

#[derive(Clone)]
struct Piece {
    source: PieceSource,
    offset: u64,
    length: u64,
}

enum PieceSource {
    Original,  // 引用原始文件
    AddBuffer, // 引用追加缓冲区
}
```

### 3.4 Viewport（视口）— 前后端协作的滑动窗口

```
文件: ████████████████████████████████████████████ 100GB
                  │
                  ▼
      ┌───────────────────────┐
      │    Viewport Window    │
      │  ┌─────────────────┐  │
      │  │ Preload Buffer   │  │  前后各预加载 200 行
      │  │  (200 lines)     │  │  保证滚动流畅
      │  ├─────────────────┤  │
      │  │ Visible Area     │  │  当前屏幕可见行
      │  │  (50-100 lines)  │  │  实际渲染到 DOM
      │  ├─────────────────┤  │
      │  │ Preload Buffer   │  │
      │  │  (200 lines)     │  │
      │  └─────────────────┘  │
      └───────────────────────┘
      总计内存: ~500行文本 ≈ 50KB
```

**视口请求协议：**

```typescript
// 前端 → 后端
interface ViewportRequest {
  startLine: number;   // 请求起始行号
  lineCount: number;   // 请求行数 (含预加载)
  reason: 'scroll' | 'jump' | 'edit' | 'init';
}

// 后端 → 前端
interface ViewportResponse {
  lines: string[];           // 文本行内容
  startLine: number;         // 实际起始行
  totalLines: number;        // 总行数 (可能为估计值)
  totalSize: number;         // 文件总大小 (bytes)
  encoding: string;          // 文件编码
  lineEndings: 'lf' | 'crlf'; // 换行符类型
  indexProgress: number;     // 索引构建进度 0.0~1.0
}
```

---

## 4. 前端架构

### 4.1 虚拟滚动 + 自定义渲染

```
┌─ Editor Container (100% height) ──────────────────┐
│ ┌──────────┬───────────────────────────────────┐   │
│ │ Line     │  Text Rendering Area              │   │
│ │ Numbers  │                                   │   │
│ │          │  ┌─────────────────────────────┐  │   │
│ │  1001    │  │  <div> line 1001 content     │  │   │
│ │  1002    │  │  <div> line 1002 content     │  │   │
│ │  ...     │  │  ...                         │  │   │
│ │  1050    │  │  <div> line 1050 content     │  │   │
│ │          │  └─────────────────────────────┘  │   │
│ │          │                                   │   │
│ │          │  ↑ padding-top: offset into file  │   │
│ │          │  ↓ padding-bottom: rest of file   │   │
│ └──────────┴───────────────────────────────────┘   │
│                                                     │
│ ┌─ Scrollbar ──────────────────────────────────┐   │
│ │ ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │   │
│ └──────────────────────────────────────────────┘   │
│                                                     │
│ ┌─ Status Bar ─────────────────────────────────┐   │
│ │ UTF-8 | LF | Line 1042, Col 15 | 50.2 GB    │   │
│ └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

**关键技术点：**

1. **DOM 虚拟化**：只渲染可见行（通常 50-100 行），其余用 padding 占位
2. **固定行高假设**：初始使用估算行高快速布局，首次渲染后测量实际行高并缓存
3. **自定义滚动条**：不依赖浏览器原生滚动（DOM 高度无法表示 100GB），使用自定义滚动条映射到行号空间
4. **滚动节流**：scroll 事件节流 16ms（60fps），避免 IPC 风暴

### 4.2 文本渲染方案选择

| 方案 | 优点 | 缺点 | 推荐场景 |
|------|------|------|----------|
| **DOM `<div>` 行渲染** | 实现简单、文本选择原生支持 | 大量行时 DOM 操作有开销 | 行数可控（虚拟化后） |
| **Canvas 渲染** | 性能极致、不受 DOM 限制 | 文本选择/光标需自行实现 | 超高性能需求 |
| **混合方案** | DOM 做可见区域 + Canvas 做预览 | 复杂度高 | 最佳体验 |

**推荐：DOM 虚拟行渲染**。虚拟化后 DOM 节点始终控制在 200 个以内，性能足够，且原生支持文本选择、光标、IME 输入法等，开发成本低。

### 4.3 前端组件结构

```
src/
├── main.tsx
├── App.tsx                      # 根组件，管理多 Tab
├── components/
│   ├── Editor/
│   │   ├── EditorView.tsx       # 编辑器主容器
│   │   ├── VirtualScroller.tsx  # 虚拟滚动控制器
│   │   ├── TextRenderer.tsx     # 文本行渲染
│   │   ├── LineNumber.tsx       # 行号栏
│   │   ├── Cursor.tsx           # 光标管理
│   │   ├── Selection.tsx        # 文本选择
│   │   └── Minimap.tsx          # 右侧缩略图 (可选)
│   ├── Menu/
│   │   ├── MenuBar.tsx          # 菜单栏
│   │   ├── ContextMenu.tsx      # 右键菜单
│   │   └── hooks/
│   │       └── useShortcuts.ts  # 快捷键
│   ├── Search/
│   │   ├── SearchBar.tsx        # 搜索栏
│   │   └── ReplaceBar.tsx       # 替换栏
│   ├── StatusBar/
│   │   └── StatusBar.tsx        # 底部状态栏
│   ├── TabBar/
│   │   └── TabBar.tsx           # 多标签栏
│   └── Dialogs/
│       ├── OpenFileDialog.tsx
│       ├── GotoLineDialog.tsx
│       └── EncodingDialog.tsx
├── hooks/
│   ├── useEditor.ts             # 编辑器核心 hook
│   ├── useViewport.ts           # 视口管理 hook
│   ├── useFile.ts               # 文件操作 hook
│   └── useSearch.ts             # 搜索 hook
├── services/
│   ├── tauriApi.ts              # Tauri IPC 封装
│   └── editorState.ts           # 编辑器状态管理 (zustand)
└── types/
    └── editor.ts                # 类型定义
```

---

## 5. 后端架构 (Rust)

### 5.1 模块结构

```
src-tauri/src/
├── main.rs
├── lib.rs                       # Tauri 入口，注册命令
├── app_state.rs                 # 应用全局状态
├── buffer/
│   ├── mod.rs
│   ├── piece_table.rs           # Piece Table 实现
│   └── line_index.rs            # 两级行索引
├── file/
│   ├── mod.rs
│   ├── mapper.rs                # mmap 文件映射
│   ├── encoding.rs              # 编码检测与转换
│   └── saver.rs                 # 文件保存
├── viewport/
│   ├── mod.rs
│   └── manager.rs               # 视口计算与文本提取
├── search/
│   ├── mod.rs
│   ├── text_search.rs           # 纯文本搜索 (Boyer-Moore)
│   └── regex_search.rs          # 正则搜索
├── syntax/
│   ├── mod.rs
│   └── highlighter.rs           # 基础语法高亮 (可选)
└── commands/
    ├── mod.rs
    ├── file_cmds.rs             # 文件相关 Tauri 命令
    ├── edit_cmds.rs             # 编辑相关 Tauri 命令
    ├── search_cmds.rs           # 搜索相关 Tauri 命令
    └── viewport_cmds.rs         # 视口相关 Tauri 命令
```

### 5.2 核心 Tauri 命令

```rust
// === 文件命令 ===
#[tauri::command]
async fn open_file(path: String, state: State<'_, AppState>) -> Result<FileInfo, String>

#[tauri::command]
async fn save_file(tab_id: String, state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn save_file_as(tab_id: String, path: String, state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn close_file(tab_id: String, state: State<'_, AppState>) -> Result<(), String>

// === 视口命令 ===
#[tauri::command]
async fn get_viewport(tab_id: String, start_line: u64, line_count: u32) -> Result<ViewportData, String>

#[tauri::command]
async fn goto_line(tab_id: String, line: u64) -> Result<ViewportData, String>

#[tauri::command]
async fn get_line_count(tab_id: String, state: State<'_, AppState>) -> Result<LineCountInfo, String>

// === 编辑命令 ===
#[tauri::command]
async fn insert_text(tab_id: String, line: u64, col: u32, text: String) -> Result<EditResult, String>

#[tauri::command]
async fn delete_range(tab_id: String, start: TextPosition, end: TextPosition) -> Result<EditResult, String>

#[tauri::command]
async fn replace_range(tab_id: String, start: TextPosition, end: TextPosition, text: String) -> Result<EditResult, String>

// === 搜索命令 ===
#[tauri::command]
async fn search(tab_id: String, query: String, options: SearchOptions) -> Result<Vec<SearchMatch>, String>

#[tauri::command]
async fn search_next(tab_id: String, search_id: String) -> Result<Option<SearchMatch>, String>

#[tauri::command]
async fn replace_all(tab_id: String, search_id: String, replacement: String) -> Result<u64, String>
```

### 5.3 应用状态管理

```rust
pub struct AppState {
    /// 打开的文档管理器
    docs: RwLock<HashMap<String, Document>>,
    /// 后台线程池
    pool: ThreadPool,
}

pub struct Document {
    /// 文件路径
    path: PathBuf,
    /// mmap 映射
    mmap: Mmap,
    /// 编辑缓冲区
    buffer: RwLock<PieceTable>,
    /// 行索引
    line_index: RwLock<LineIndex>,
    /// 文件元信息
    info: FileInfo,
    /// 是否已修改
    dirty: AtomicBool,
    /// 索引构建进度
    index_progress: Arc<AtomicU32>, // 0-1000 表示 0.0%-100.0%
}
```

---

## 6. 关键流程

### 6.1 打开大文件流程

```
用户选择文件
     │
     ▼
[Rust] 检测编码，建立 mmap 映射     ← < 100ms
     │
     ▼
[Rust] 扫描前 1MB，构建初始行索引    ← < 50ms
     │
     ▼
[Frontend] 立即显示前 N 行           ← 用户无感知延迟
     │
     ▼
[Rust] 后台线程逐块扫描文件          ← 可中断
     │              │
     │   定期发送进度事件 ──→ [Frontend] 更新状态栏进度
     │              │
     ▼              ▼
[Rust] 索引构建完成 ──→ [Frontend] 滚动条变为精确模式
```

### 6.2 滚动浏览流程

```
用户滚动鼠标 / 拖拽滚动条
     │
     ▼
[Frontend] 节流 16ms，计算目标行号
     │
     ▼
[Frontend] → Tauri IPC → get_viewport(tab_id, start_line, count)
     │
     ▼
[Rust] 通过行索引定位文件偏移
     │
     ▼
[Rust] 从 mmap 读取对应区域的文本  ← OS page cache, 极快
     │
     ▼
[Rust] 解码 + 按行切割 + 返回      ← < 5ms
     │
     ▼
[Frontend] 更新 DOM 中可见行文本    ← < 5ms
```

### 6.3 编辑流程

```
用户在行 1050, 列 20 处输入 "Hello"
     │
     ▼
[Frontend] → Tauri IPC → insert_text(tab_id, 1050, 20, "Hello")
     │
     ▼
[Rust] 计算逻辑偏移 (通过行索引 + Piece Table)
     │
     ▼
[Rust] 更新 Piece Table:
        - 将 "Hello" 追加到 Add Buffer
        - 在 Piece 描述符中插入新 Piece
        - 更新行索引缓存
     │
     ▼
[Rust] 返回新的视口数据
     │
     ▼
[Frontend] 更新渲染 + 标记文档已修改
```

### 6.4 保存流程

```
用户 Ctrl+S 保存
     │
     ▼
[Rust] 检查 Piece Table 是否有编辑
     │
     ├─ 无编辑 → 直接返回 (文件未修改)
     │
     ▼ 有编辑
[Rust] 写入临时文件:
        遍历 Piece Table，按顺序写出
        Original Piece → 从 mmap 读取写出
        AddBuf Piece  → 从 Add Buffer 读取写出
     │
     ▼
[Rust] 原子重命名: tmp_file → 原文件
     │
     ▼
[Rust] 重新 mmap，重建索引
     │
     ▼
[Rust] 清空 Add Buffer，重置 Piece Table
```

---

## 7. 性能预算

### 7.1 内存占用目标

| 场景 | 目标内存 | 说明 |
|------|----------|------|
| 打开 100GB 文件，不操作 | < 50MB | mmap 映射开销 + 主索引 |
| 浏览滚动中 | < 80MB | 加上视口预加载缓冲 |
| 编辑中 (少量编辑) | < 100MB | 加上 Add Buffer |
| 编辑中 (大量编辑) | < 200MB | Add Buffer 增长，可配置阈值触发自动保存 |

### 7.2 响应时间目标

| 操作 | 目标延迟 | 说明 |
|------|----------|------|
| 打开文件到可见 | < 200ms | 初始行扫描完成 |
| 滚动一屏 | < 30ms | IPC + 渲染 |
| 跳转到文件头/尾 | < 100ms | 索引查找 + 加载 |
| 搜索 (已索引) | < 1s / GB | 后台搜索，流式返回结果 |
| 输入单个字符 | < 10ms | Piece Table 更新 |

---

## 8. Rust 依赖选择

```toml
[dependencies]
# 框架
tauri = { version = "2", features = [] }
tauri-plugin-log = "2"
tauri-plugin-dialog = "2"      # 文件选择对话框
tauri-plugin-fs = "2"          # 文件系统访问

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 内存映射
memmap2 = "0.9"                # 文件 mmap

# 编码检测与转换
encoding_rs = "0.8"            # 编码支持 (UTF-8, GBK, Shift_JIS 等)
chardetng = "0.1"              # 编码检测

# 搜索
aho-corasick = "1"             # 多模式字符串搜索
regex = "1"                    # 正则搜索

# 并发
rayon = "1"                    # 并行计算 (索引构建、搜索)
crossbeam-channel = "0.5"      # 高性能通道 (进度通知)

# 数据结构
lru = "0.12"                   # LRU 缓存 (块索引缓存)

# 日志
log = "0.4"
```

---

## 9. 开发路线图

### Phase 1: 基础框架 (1-2 周)

- [ ] Rust 侧: 文件 mmap 映射 + 编码检测
- [ ] Rust 侧: 基础行索引 (全量扫描，先不做两级优化)
- [ ] Rust 侧: 基础 Tauri 命令 (open, get_viewport, get_line_count)
- [ ] 前端: 编辑器基本布局 (行号 + 文本区 + 状态栏)
- [ ] 前端: 虚拟滚动 MVP
- [ ] 联调: 能打开文件并浏览

### Phase 2: 编辑能力 (1-2 周)

- [ ] Rust 侧: Piece Table 实现
- [ ] Rust 侧: 编辑命令 (insert, delete, replace)
- [ ] Rust 侧: 文件保存
- [ ] 前端: 光标与文本选择
- [ ] 前端: 键盘输入处理
- [ ] 前端: IME 输入法支持

### Phase 3: 超大文件优化 (1-2 周)

- [ ] Rust 侧: 两级稀疏行索引
- [ ] Rust 侧: 后台索引构建 + 进度通知
- [ ] Rust 侧: 视口预加载优化
- [ ] 前端: 自定义滚动条
- [ ] 前端: Goto Line 对话框
- [ ] 测试: 用脚本生成 10GB+ 测试文件验证

### Phase 4: 搜索与替换 (1 周)

- [ ] Rust 侧: 流式文本搜索 (Boyer-Moore)
- [ ] Rust 侧: 正则搜索
- [ ] Rust 侧: 搜索结果流式返回
- [ ] 前端: 搜索/替换 UI
- [ ] 前端: 搜索结果高亮与导航

### Phase 5: 完善体验 (持续)

- [ ] 多标签支持
- [ ] 拖放打开文件
- [ ] 最近文件记录
- [ ] 基础语法高亮 (可选)
- [ ] 右侧 Minimap (可选)
- [ ] 自定义主题 (可选)
- [ ] 性能监控面板 (开发调试用)

---

## 10. 风险与对策

| 风险 | 影响 | 对策 |
|------|------|------|
| 超长行 (单行 > 1GB) | 行索引失效，渲染卡顿 | 长行自动折行显示，限制单次加载长度 |
| mmap 在网络/移动存储上表现差 | I/O 延迟不可控 | 检测文件系统类型，网络盘回退到普通 I/O |
| 编码检测不准 | 乱码 | 提供手动切换编码功能，优先信任 BOM |
| 频繁编辑导致 Piece Table 膨胀 | 内存增长 | 超过阈值自动 flatten (重建连续 Piece) |
| 前端虚拟滚动与输入法冲突 | 光标位置错乱 | 使用隐藏的 contenteditable div 捕获输入 |
| 32 位系统地址空间不足 | 无法 mmap 大文件 | 检测平台，32 位系统使用分块读取 |

---

## 11. 与同类产品的技术对比

| 特性 | NotePadA | VS Code | Notepad++ | EmEditor |
|------|----------|---------|-----------|----------|
| 技术栈 | Tauri(Rust+Web) | Electron | Win32 C++ | Win32 C++ |
| 大文件支持 | mmap + Piece Table | 原生不支持 | 基础支持 | 优秀 (商业) |
| 内存占用目标 | < 100MB | > 300MB | < 50MB | < 200MB |
| 跨平台 | Win/Mac/Linux | Win/Mac/Linux | 仅 Windows | 仅 Windows |
| 启动速度 | < 500ms | > 2s | < 100ms | < 500ms |

**差异化定位：** 以 Notepad++ 的轻量为目标，以 EmEditor 的大文件能力为标杆，以跨平台和现代 UI 为优势。
