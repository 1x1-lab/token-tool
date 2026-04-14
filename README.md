# ZhipuKit

一款基于 Tauri 2 + Vue 3 构建的轻量桌面工具，用于查询智谱 AI（Zhipu / Z.ai）账户余额、Coding Plan 配额、Token 用量计算，以及管理本机 Claude Code CLI 配置。

## 功能

### 余额查询
- 账户余额（当前余额、可用余额、冻结余额）
- 累计充值、赠送金额、累计消费
- Coding Plan 配额
  - 5 小时 Token 限额（使用百分比 + 重置倒计时）
  - 每周边额（部分套餐生效）
  - MCP 月度调用次数（已用/总量）
- 支持自动刷新（10 秒 ~ 5 分钟可调）

### Token 计算
- 支持多模型选择（GLM-4-Plus / GLM-4 / GLM-4-Air / GLM-4-Long / GLM-4-Flash / GLM-4.5 等）
- 本地快速估算 + API 精确计算
- 预估费用计算

### Claude Code 配置
- 自动检测本机 Claude Code CLI 安装状态、版本号、路径（支持多路径探测 + Shell 检测双重机制）
- 读取并展示 `~/.claude/settings.json` 配置文件
- 可编辑配置：
  - 默认模型（model）
  - API 密钥（ANTHROPIC_AUTH_TOKEN），支持一键填入
  - API 端点（ANTHROPIC_BASE_URL）
  - 模型映射（Haiku / Sonnet / Opus）
  - 超时设置（API_TIMEOUT_MS）
- 安全读写，保留未知配置字段不变

### 设置
- API Key 管理（本地存储）
- 支持国内版（open.bigmodel.cn）和国际版（api.z.ai）
- 自动刷新倒计时显示
- 关闭到托盘开关
- 开机自启动
- 调试工具（API 连通性测试）
- 开发者工具（DevTools、应用信息）

### 系统托盘
- 点击托盘图标弹出快捷面板，显示余额和额度概览
- 双击托盘图标显示主窗口
- 关闭窗口最小化到托盘继续运行
- 开机自启时直接后台运行，不闪现窗口

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | Vue 3 + TypeScript + Vite |
| 后端 | Rust + Tauri 2 |
| HTTP | reqwest |
| 异步 | tokio |

## 开发

```bash
# 安装依赖
npm install

# 开发模式
npm run tauri dev

# 构建生产包
npm run tauri build
```

### 前置要求

- Node.js >= 18
- Rust >= 1.77
- Tauri CLI 2.x

## 项目结构

```
src/                      # 前端源码
  App.vue                 # 主布局（侧边栏 + 页面切换、关闭确认对话框）
  main.ts                 # 入口
  TrayPopup.vue           # 系统托盘弹窗
  tray-popup-main.ts      # 托盘弹窗入口
  views/
    BalanceQuery.vue      # 余额查询页面
    TokenCalculator.vue   # Token 计算页面
    ClaudeConfig.vue      # Claude Code 配置页面
    SettingsView.vue      # 设置页面
  composables/
    useBalanceCache.ts    # 余额数据缓存与自动刷新事件监听
src-tauri/
  src/
    lib.rs                # 应用入口（窗口管理、托盘、开机自启隐藏）
    api.rs                # API 调用、自动刷新定时器
    claude.rs             # Claude Code 检测与配置管理
    tray.rs               # 托盘交互、关闭到托盘设置持久化
    types.rs              # 共享类型定义
    utils.rs              # 工具函数
  tauri.conf.json         # Tauri 配置
  Cargo.toml              # Rust 依赖
```

## API 端点

| 功能 | 端点 |
|---|---|
| 余额查询 | `GET /api/biz/account/query-customer-account-report` |
| Coding Plan 配额 | `GET /api/monitor/usage/quota/limit` |
| Token 计算 | `POST /api/paas/v4/chat/completions` |

## License

MIT
