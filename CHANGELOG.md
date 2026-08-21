## 1.0.0 (2026-08-21)

### Features

* add agent context sidebar backend (aggregated context, LSP detection) ([27f2816](https://github.com/laochenfei233/Prism-Agent-R/commit/27f2816ab29d52f2d04514fd8af0556456b7841b))
* add agent sidebar with 6 tabs (usage, workdir, instructions, mcp, lsp, files) ([7e9a1b5](https://github.com/laochenfei233/Prism-Agent-R/commit/7e9a1b5d2df027d76eef1dc97af07183459816b8))
* add dashboard overview backend (aggregated stats, agents, sessions) ([627d134](https://github.com/laochenfei233/Prism-Agent-R/commit/627d134e2a331d592f3e1d0a7382bfee3ca54416))
* add home dashboard page (stats, agent launcher, trends) ([b086ba9](https://github.com/laochenfei233/Prism-Agent-R/commit/b086ba90db416659797acc7c7968dee03c256e5b))
* add more ASR models and apply glass materials to dashboard ([1a90821](https://github.com/laochenfei233/Prism-Agent-R/commit/1a9082120b9893e38f5aef0173def335b07e1b9d))
* add Phase 1 core implementation (ADK/Rig/AutoAgents, MCP, services, commands) ([a7adbe8](https://github.com/laochenfei233/Prism-Agent-R/commit/a7adbe8f9cc970cacc134c48a4b7bf079292dae7))
* add session title FTS search (migration 012) ([2c5d6fc](https://github.com/laochenfei233/Prism-Agent-R/commit/2c5d6fc3fec6ce9b565693128c0c48b8cef98a6f))
* add task definition backend and IPC commands ([1275f61](https://github.com/laochenfei233/Prism-Agent-R/commit/1275f611b1ba2dcea95f372c3e3eb2a6f1d178b6))
* add task designer (template/design/run modes, canvas, validation) ([0989869](https://github.com/laochenfei233/Prism-Agent-R/commit/098986988d353fe3a4945a09503e7a698eface0a))
* add tool approval dialog and skill market UI ([daa820a](https://github.com/laochenfei233/Prism-Agent-R/commit/daa820a44cad2a09500674916b6cb006244e2284))
* add tool approval flow (HITL) with risk assessment ([c689136](https://github.com/laochenfei233/Prism-Agent-R/commit/c689136160b00980292b6080d53d4c8868fca242))
* ADK component layer + Rig core layer ([f33c109](https://github.com/laochenfei233/Prism-Agent-R/commit/f33c1098116547125b0803f03bbc14a10f2f7f94))
* **agent:** add file read/write/list tools to Agent tool registry ([4cfac2d](https://github.com/laochenfei233/Prism-Agent-R/commit/4cfac2d59f47e071ec8ce0cf5415bd4ebb0db6c7))
* **agent:** auto-assign default thinking model to new agents ([a5bf055](https://github.com/laochenfei233/Prism-Agent-R/commit/a5bf0554518bbcea583b926fff975a20d62890bd))
* **agents:** 内置 8 个 OPC Agent 懒加载种子, 移除默认「助手」创建逻辑 ([452ccbf](https://github.com/laochenfei233/Prism-Agent-R/commit/452ccbf29f4f6509fb4dc43ceafa39d9ab95d4b0))
* **asr:** add online/local model categories ([c72a3e0](https://github.com/laochenfei233/Prism-Agent-R/commit/c72a3e085f8753b544255a8ec18d3139398b5e9a))
* **chat:** markdown-it + shiki rendering, buffered streaming, thinking UI ([eeea753](https://github.com/laochenfei233/Prism-Agent-R/commit/eeea753e6820749e8ce37a4c9617a0628d989fc3))
* **compose:** add Build mode as default, replace single toggle with mode selector ([fab128a](https://github.com/laochenfei233/Prism-Agent-R/commit/fab128a430db96ee55c25834d535f492dbff9e54))
* **compose:** add Compose mode for single-agent orchestration ([6ad6f77](https://github.com/laochenfei233/Prism-Agent-R/commit/6ad6f7702b2dfd8bcb76fe5ba8c6ac87e89d29f9))
* implement skill market search (3-source concurrent fetch) ([0926dec](https://github.com/laochenfei233/Prism-Agent-R/commit/0926dec0a34a99a3d428a9e76deba4602959bb55))
* **kanban:** add Agent task management tools + tasks display ([294626e](https://github.com/laochenfei233/Prism-Agent-R/commit/294626ee38a7a24edbd139fc9501528d470f18bf))
* **layout:** 三段式 Cherry Studio 布局——最左窄导航 + 面板/Agent 拆分 ([c7c8f05](https://github.com/laochenfei233/Prism-Agent-R/commit/c7c8f051ef28ac4f54b358d11e5f78c85f7f144f))
* **meetings:** 参考 prism-agent 重构会议页面——主从布局 + 实时转录 + Q&A ([9971666](https://github.com/laochenfei233/Prism-Agent-R/commit/99716662f381f723b59dda9a0693269161205f32))
* MVP scaffold — Tauri 2.x + Svelte 5 + design system + database layer ([b7f426e](https://github.com/laochenfei233/Prism-Agent-R/commit/b7f426e70f2a7b2d50f53e981679412f522b40f0))
* **panel:** filter promptless agents + per-status column containers ([514f625](https://github.com/laochenfei233/Prism-Agent-R/commit/514f625ac10bf059f8935b91e34630bb1983f5b5))
* **panel:** panel refactor & settings modern design, resolve clippy gate ([6e36852](https://github.com/laochenfei233/Prism-Agent-R/commit/6e3685239b6795b60371440dfca9608b13cf442b))
* **panel:** replace flat grid with Session Kanban board ([9dd40d0](https://github.com/laochenfei233/Prism-Agent-R/commit/9dd40d007e36924c112662ac79e655076c223a9c))
* **panel:** 自主编排接入面板主入口 + 子任务状态卡片与任务级事件 ([b779554](https://github.com/laochenfei233/Prism-Agent-R/commit/b7795541e4b602bbd9837fc328c62228778a98fc))
* Phase 1 缺口补齐 — MCP 接入 Agent / API Key 加密 / 记忆 FTS5 / 文件域 / 聊天组件 ([f91f98b](https://github.com/laochenfei233/Prism-Agent-R/commit/f91f98b61a05819b3952db57d216c0ed6359d1b7))
* Phase 2 修复 — 打通执行引擎/HITL接线/用量管道/流式响应/市场安装/侧边栏命令/数据底座 ([1888bcb](https://github.com/laochenfei233/Prism-Agent-R/commit/1888bcbd3ace59d40ea08375bf48ddee061008f9))
* Phase 3 扩展功能完成 — Wiki/RAG（五维评测+报告趋势+项目自动索引）/会议（说话人分离+8后端）/翻译OCR/反思/护栏/目标/评估/上下文压缩/Router ([5772c4e](https://github.com/laochenfei233/Prism-Agent-R/commit/5772c4e7c96d3cfa581b79ebb227ad9a8fc7ba79))
* Phase 4 Harness 工程化（T8-T10） ([bc2a7df](https://github.com/laochenfei233/Prism-Agent-R/commit/bc2a7dff20ce715957403b21ff2307db3dfccaa5))
* Phase 4 前端 UI 任务（T-F1~T-F5） ([027a7c7](https://github.com/laochenfei233/Prism-Agent-R/commit/027a7c75108e48914a0b99fa2cf75f299a163d3c))
* Phase 4 增量设计（T12-T14） ([6e2f827](https://github.com/laochenfei233/Prism-Agent-R/commit/6e2f8270e27f478be12368819a3b146fab70dec4))
* Phase 4 增量设计（T15-T19）- 全部完成 ([0696ce3](https://github.com/laochenfei233/Prism-Agent-R/commit/0696ce364e20de3a6b4c24417490be316c12d81c))
* Phase 4 核心后端实现（T1-T7） ([a0a859a](https://github.com/laochenfei233/Prism-Agent-R/commit/a0a859afacecad92b578d415810a22d891395bd4))
* **phase5:** §27.4/§27.6 编排交互——暂停/继续/终止命令 + 前端控制按钮 ([309ef02](https://github.com/laochenfei233/Prism-Agent-R/commit/309ef02ffa178f8253fdbed9f200644a7d2c3b42))
* **phase5:** complete T12/T15/T20 - workflow V2 migration, monitor controls, orchestrator UI ([18a2bd2](https://github.com/laochenfei233/Prism-Agent-R/commit/18a2bd217bd4dce5f258e55bb65a340498508520))
* **phase5:** production hardening - budget, guardrails, observability, orchestrator ([2332265](https://github.com/laochenfei233/Prism-Agent-R/commit/2332265a4c242b4134bc60229221b816316bf137))
* redesign dashboard — OpenAI-style, workflow first, compact 3-column stats ([978d34c](https://github.com/laochenfei233/Prism-Agent-R/commit/978d34c3974b2dcd28f6c885e2999634b59c59e5))
* redesign TaskDesigner — canvas-based workflow editor with templates, drag-reorder, OpenAI style ([18b16c8](https://github.com/laochenfei233/Prism-Agent-R/commit/18b16c85529fa43d084881ae6c738d1393b5dad2))
* **settings:** LLM 模型管理布局优化 ([a964404](https://github.com/laochenfei233/Prism-Agent-R/commit/a964404a9de01f4788ddcb9b8b18c924badd22fd))
* **settings:** modernize LLM/ASR/TTS with iOS 26 Liquid Glass design ([d2b2a3c](https://github.com/laochenfei233/Prism-Agent-R/commit/d2b2a3c299eec93fde68c302687228e034b753ee))
* **settings:** 中间栏罗列全部预置供应商 ([84eed83](https://github.com/laochenfei233/Prism-Agent-R/commit/84eed83166950ae862acdebe120fd3d771dd9b3a))
* **settings:** 仅配置 Key 且地址有效时自动拉取模型 ([4c1e5a1](https://github.com/laochenfei233/Prism-Agent-R/commit/4c1e5a1911355cdbed39353fbc75ebdb500b0bb9))
* **settings:** 模型管理区块 Cherry Studio 风格重做 ([eff63a7](https://github.com/laochenfei233/Prism-Agent-R/commit/eff63a748ed086b2efe3de7cc3694802b98a4cbf))
* **settings:** 模型管理重构——Provider列窄化+分割线，ASR并入两栏，新增TTS ([768f9df](https://github.com/laochenfei233/Prism-Agent-R/commit/768f9df581ff6bf488daad34c1d33523174dfc62))
* **settings:** 模型预置可选 + 供应商图标自定义 + 修复白底图标 ([0580e0e](https://github.com/laochenfei233/Prism-Agent-R/commit/0580e0e07e643924297501929be1e8d0b1596f15))
* **settings:** 移植 Cherry Studio 17 个供应商预设 + SVG Logo ([93dc3d7](https://github.com/laochenfei233/Prism-Agent-R/commit/93dc3d7b38a59088f9f2459cb90d6a3b64874210))
* **settings:** 设置页 Cherry Studio 风格重设计 + 间距精修, 移除知识库/设置页返回键 ([324e8d4](https://github.com/laochenfei233/Prism-Agent-R/commit/324e8d42728cd95619cbb3a4c00a1a4138a3e0df))
* **settings:** 设置页中间栏 Cherry Studio 风格重做 ([a973de6](https://github.com/laochenfei233/Prism-Agent-R/commit/a973de6f9e5f95374393f6677662db1dcbd1bbb2))
* sherpa-rs 改为可选 feature（sherpa-native，默认关闭）——Agent 本体零依赖构建运行 ([147819f](https://github.com/laochenfei233/Prism-Agent-R/commit/147819ffde0714a1ec4ecf56c92ead35bee689b6))
* T6 服务层 + T11 对话前端 MVP ([200d507](https://github.com/laochenfei233/Prism-Agent-R/commit/200d507b11f8e2101c6b71cb88da88b916ee5e2f))
* **tests:** Agent 能力检测套件（对话/运行/本地读写闭环）+ Windows 测试 manifest ([1039e44](https://github.com/laochenfei233/Prism-Agent-R/commit/1039e44cd9e0c824fe3718306399d2fef06151f6))
* **ui:** Agent 侧边栏仅聊天页显示 + 设置页 Cherry Studio 风格重设计 ([d3487c4](https://github.com/laochenfei233/Prism-Agent-R/commit/d3487c4080042921c7a0e9c07f4c1ff3b6399022))
* **ui:** 翻译页与知识库页三段式布局 + 知识库全宽显示 ([983075d](https://github.com/laochenfei233/Prism-Agent-R/commit/983075d251bb1c02c18d721df9b600e3bd752c60))
* **wiki+translate:** 参考 Cherry Studio 对应模块重新设计 ([5673e5e](https://github.com/laochenfei233/Prism-Agent-R/commit/5673e5ea40f8c7c5d900ad8c8549f7ef2153b652))
* **wiki:** 知识库页 Cherry Studio 风格重设计 + 返回键 ([5be33b3](https://github.com/laochenfei233/Prism-Agent-R/commit/5be33b393df79a00c2a63447d8bd89f40cdcf31a))
* 主页直接集成快速配置（无需跳转设置页） ([474c571](https://github.com/laochenfei233/Prism-Agent-R/commit/474c571f656d6a0db68582c6963da11fd1eabd2b))
* 主页集成对话界面 + 完整流程 ([dfc917f](https://github.com/laochenfei233/Prism-Agent-R/commit/dfc917f83d4824c6485341c70bfb13be3c3071ef))
* 优化画布节点设计 — 彩色角色标签、依赖标注、渐变图标、连接线增强 ([22cb95b](https://github.com/laochenfei233/Prism-Agent-R/commit/22cb95bb77d0cf09aebd1bfc75ea7c17f77cad69))
* 低优先级缺口补齐 — 记忆 trigram / 迁移机制 / 快捷键 / 流式渲染 ([4950c85](https://github.com/laochenfei233/Prism-Agent-R/commit/4950c859a795825d5f27e849d8f93dab3bb58481))
* 全应用回归 Apple Design — token 化、双模式主题、去 AI 味 ([6db364c](https://github.com/laochenfei233/Prism-Agent-R/commit/6db364c6af317dfebe55e6972b6e2cd6eae49023))
* 全部转为中文 — 工作流模板、画布节点、按钮文案 ([3909b80](https://github.com/laochenfei233/Prism-Agent-R/commit/3909b80d5bb86832a1dfaeb93e22f6baeb9959c5))
* 剩余中低优先项 — 侧边栏交互 / 会话搜索入口 / 事件增量刷新 ([39c45d7](https://github.com/laochenfei233/Prism-Agent-R/commit/39c45d747c5ba4631ef20b0f85ff8946bf51f0b4))
* 完整对话流程 + 设置页面 ([7e4bb97](https://github.com/laochenfei233/Prism-Agent-R/commit/7e4bb97221c4fb27146cdd460020122b8e0f8487))
* 对话页面添加返回面板按钮（←） ([cf1152f](https://github.com/laochenfei233/Prism-Agent-R/commit/cf1152f448576d7689ac895e7413a2e5950ba0d9))
* 左侧栏 logo 点击返回 Dashboard ([e4dbe31](https://github.com/laochenfei233/Prism-Agent-R/commit/e4dbe31e37c51d48b8709855e19737d68905042d))
* 接入 13 个预留命令到前端（P1/P2 收尾） ([fed8f24](https://github.com/laochenfei233/Prism-Agent-R/commit/fed8f2463cf95e7dc254e1e5499925c5f5000132))
* 收尾四项 — TTS 播报（§10.3.9）/CI 评测回归门槛（§10.2.5）/Azure WS 流式（§10.3.3⑦）/CI 测试与 Linux 打包依赖 ([eac08bd](https://github.com/laochenfei233/Prism-Agent-R/commit/eac08bdb93372a9f488b1a9b56327926bdf4218e))
* 设置中心重构 — 全量可配置项注册表 + 统一 preferences 读写 + 设置页八组重新设计 ([6574306](https://github.com/laochenfei233/Prism-Agent-R/commit/65743064efacd91b5335057bf8973162df9ddba1))

### Bug Fixes

* **a11y:** HIGH 级修复 H1/H2/H3/H4/H5/H9 ([6181aa1](https://github.com/laochenfei233/Prism-Agent-R/commit/6181aa1fe809d6185f41a280823e2b236897d3f9))
* **a11y:** MEDIUM 级——骨架屏/aria-pressed/disabled态/命名一致性 ([152cc0c](https://github.com/laochenfei233/Prism-Agent-R/commit/152cc0c9f8318f4e9f39ae4bac6d78ca6a7282bd))
* **a11y:** 三段式布局后的 HIGH 级 a11y 清理——警告 37→8 ([f180ddb](https://github.com/laochenfei233/Prism-Agent-R/commit/f180ddb33addb6211aaecfd50c80257e971826c5))
* **a11y:** 剩余待处理——H7/H8 对比度 + MEDIUM 项 ([af5a614](https://github.com/laochenfei233/Prism-Agent-R/commit/af5a614c41d224f65c2df4fd1868dc185384819c))
* **a11y:** 审计修复 C3/C1/C2——添加Provider可达 + 删除按钮+知识库键盘可达 ([3d2c8c1](https://github.com/laochenfei233/Prism-Agent-R/commit/3d2c8c18d10a7ff77a5f25d4cfe8a20f271407e5))
* **a11y:** 重设计后 HIGH 级 a11y 复检修复 ([dad21e9](https://github.com/laochenfei233/Prism-Agent-R/commit/dad21e918b24de971ac77017e3c13581b6737318))
* Agent add button always visible + session add button ([912e996](https://github.com/laochenfei233/Prism-Agent-R/commit/912e996354f024e4ac9f6209057c4b46d6b4e384))
* Agent API 参数名全部改为 camelCase ([d5c2a61](https://github.com/laochenfei233/Prism-Agent-R/commit/d5c2a616ad387074096faaf2c2b7b3d1605fa604))
* **agent:** map snake_case to camelCase for Tauri 2.x command args ([b2b1bd1](https://github.com/laochenfei233/Prism-Agent-R/commit/b2b1bd1628c118955b978a203d3fb62f631aadff))
* align Phase 2 component styles with design system tokens ([82a7a81](https://github.com/laochenfei233/Prism-Agent-R/commit/82a7a816729ec5c4ac0de7bd405430ca5f3389ba))
* always show Dashboard (remove provider/model gate), add setup banner fallback ([ea3eeab](https://github.com/laochenfei233/Prism-Agent-R/commit/ea3eeabb7c125710240f3339d687d03ec08ba08e))
* **asr:** re-export AsrModelCategory and add missing struct fields ([a287ef9](https://github.com/laochenfei233/Prism-Agent-R/commit/a287ef9af7a70ffed6c8b1542d32dc90bdccadd5))
* **chat:** fallback model lookup by model_id string ([63dac7f](https://github.com/laochenfei233/Prism-Agent-R/commit/63dac7f0a29877687b8221b173fbe5ca3db599f2))
* **chat:** fix SSE parsing for MiMo API responses ([00b8e1e](https://github.com/laochenfei233/Prism-Agent-R/commit/00b8e1e6a79a54e4804588faa7d8d8477232d852))
* content 区域 overflow: hidden → overflow-y: auto，允许滚动 ([1f679e5](https://github.com/laochenfei233/Prism-Agent-R/commit/1f679e51bfcf7d71549eb159c5b896707803fe93))
* CSS 语法错误 + 主页快速配置引导 ([ccf1fbd](https://github.com/laochenfei233/Prism-Agent-R/commit/ccf1fbdc6d00066125097d015cbe60b14718d687))
* import app.css in layout (tokens were not loaded) ([b195f45](https://github.com/laochenfei233/Prism-Agent-R/commit/b195f450e6a35f1276c394635d15a0359a84d0f8))
* logo 显示 + 更新 spec 新会话创建流程 ([8d24604](https://github.com/laochenfei233/Prism-Agent-R/commit/8d246042857efcddd8cbd03340cb37c2d0952f32))
* **meetings:** append_recording 写后加 flush，消除录音时长偶发少算 ([cf0ce55](https://github.com/laochenfei233/Prism-Agent-R/commit/cf0ce55963492fa977c34a6bdbf1e23441f2dbcf))
* orange theme, simplified sidebar (3 tabs), unified icons, dashboard agent connection ([ee54c30](https://github.com/laochenfei233/Prism-Agent-R/commit/ee54c303d2d916ba46d7007a856f3a01df658104))
* **orchestrator:** GroupKind 兼容旧持久化 plan 的 PascalCase 反序列化 ([a051f62](https://github.com/laochenfei233/Prism-Agent-R/commit/a051f62f98b6bf664350d7862a6ae8e5fc95e8c4))
* **orchestrator:** IPC 桥补传事件 data + GroupKind snake_case 序列化 + 事件列表唯一 key ([c6bf693](https://github.com/laochenfei233/Prism-Agent-R/commit/c6bf693d70f1987fd93026d28d0dae8818dd82e5))
* **phase3:** 修复会议/知识库/翻译实现缺陷并新增内置术语库一键导入 ([5c11fda](https://github.com/laochenfei233/Prism-Agent-R/commit/5c11fda5485a3fe59437b27198b0dde32503c713))
* **phase5:** svelte class: 指令不支持 tailwind 透明度后缀，改用字符串拼接 ([34438a3](https://github.com/laochenfei233/Prism-Agent-R/commit/34438a33a0898f324429fd5f35d2fbed6e3548e1))
* **phase5:** 合并 phase4 后修复——迁移重编号 026 + V2 引擎注册 web_search 工具 ([827fcfe](https://github.com/laochenfei233/Prism-Agent-R/commit/827fcfef6f9145999f706c0f8a9a882b0411dd28))
* **phase5:** 合并后复查修复——补齐文档设计缺口（预算事件接线/模型降级/沙箱/轨迹护栏/编排持久化+LLM/前端交互） ([03e7bb2](https://github.com/laochenfei233/Prism-Agent-R/commit/03e7bb23815af15056840403455f201b2a08612a))
* remove remaining hardcoded blue color references ([c7b1c51](https://github.com/laochenfei233/Prism-Agent-R/commit/c7b1c51263d995eb43aa435d15973b77d8575daf))
* remove unused setup wizard code and CSS, clean up +page.svelte ([6761f74](https://github.com/laochenfei233/Prism-Agent-R/commit/6761f748c4c08d8da8ee1e958a8ad07e22b038ce))
* resolve 167 lint issues and format frontend sources ([54df240](https://github.com/laochenfei233/Prism-Agent-R/commit/54df240021e3a2961859cd311f0e8d933df08c08))
* resolve lint/format gates on master (102 errors, prettier normalization, eslint worktrees ignore, changelog v8) ([787a58a](https://github.com/laochenfei233/Prism-Agent-R/commit/787a58a67720084e57866963f32302527468ecac))
* **responsive:** C4 375px 响应式——固定宽布局加断点兜底 ([fb37f15](https://github.com/laochenfei233/Prism-Agent-R/commit/fb37f15508adf45ab7d451441b77e0c73d13268a))
* satisfy clippy 1.98 lint gates (sort_by_key, as_chunks, redundant into_iter) ([e775b9b](https://github.com/laochenfei233/Prism-Agent-R/commit/e775b9ba4e79dfb90f8fc36c47ddd5abcdda41e6))
* **settings:** add border frame around model list section ([6284549](https://github.com/laochenfei233/Prism-Agent-R/commit/62845490abc6ef74fd30d1872c8ea7b67a29ede0))
* **settings:** ASR middle column now filters by selected backend ([53b2a67](https://github.com/laochenfei233/Prism-Agent-R/commit/53b2a675b660eaa258dede3c7acd1613b503cef3))
* **settings:** dedup check when adding provider ([4c9f447](https://github.com/laochenfei233/Prism-Agent-R/commit/4c9f447363d210133b743b40f705613ee57a792d))
* **settings:** eye button now enters edit mode with text visible ([603df2c](https://github.com/laochenfei233/Prism-Agent-R/commit/603df2c1bc7cadb99b7bcfda138f8e9e13c59193))
* **settings:** increase padding in model list for better readability ([d1ed29c](https://github.com/laochenfei233/Prism-Agent-R/commit/d1ed29c9e0be65cda15541548517dec3dded5594))
* **settings:** model list borders, ASR backend click, TTS full-width ([463064d](https://github.com/laochenfei233/Prism-Agent-R/commit/463064d54a703386432b206158f65279d6205706))
* **settings:** 修复图标渲染 + 删除模型预设chips + 模型列表分组框 ([f2b119c](https://github.com/laochenfei233/Prism-Agent-R/commit/f2b119cd38bcc93fa08af62cd4092a7198dbba54))
* **settings:** 修复模型拉取参数 + Base URL 可编辑 + 右侧贴边 ([42474ba](https://github.com/laochenfei233/Prism-Agent-R/commit/42474ba3598fa662f961824504cc92c6303f1327))
* **settings:** 修复豆包图标渲染 + 添加模式下显示模型列表 ([f2c27b4](https://github.com/laochenfei233/Prism-Agent-R/commit/f2c27b4d73343a46f1e436593a6e802f6c08b3c7))
* **settings:** 加回返回按钮 + Provider 与模型合并（Cherry Studio 两栏） ([32f6822](https://github.com/laochenfei233/Prism-Agent-R/commit/32f6822ed761bbbdb64be61fa9b0e2d8458e0060))
* **settings:** 模型列表框始终显示(无 Key 也展示空框+提示) ([8ba240a](https://github.com/laochenfei233/Prism-Agent-R/commit/8ba240ab81aaab25a4f7c72b6ad19742bded9532))
* **settings:** 添加 Provider 模式改为与已添加视图一致的平铺设计 ([2ac6651](https://github.com/laochenfei233/Prism-Agent-R/commit/2ac6651ffe2aa80e1a66d51dd9be4a2503038baa))
* taskStore 模块级 \ 触发 effect_orphan，改为普通函数挂载事件监听 ([8804959](https://github.com/laochenfei233/Prism-Agent-R/commit/8804959be5210584a2f0bb0355a60a1284d8dd00))
* Tauri 2.x 会转换参数名 snake_case → camelCase ([c70f623](https://github.com/laochenfei233/Prism-Agent-R/commit/c70f623a07171a47eb272fd50a9829a001a84856))
* Tauri 命令名从 snake_case 改为 kebab-case ([42fe1f4](https://github.com/laochenfei233/Prism-Agent-R/commit/42fe1f460a1f9f251c8b5aff9be45b0736c19b3b))
* **ui:** logo 返回主面板 + 工具页面加宽 + ASR 设置移入设置页 ([71aa854](https://github.com/laochenfei233/Prism-Agent-R/commit/71aa85487fba3b0682959577b5278112b38232d8))
* **ui:** 左侧 Agent 侧边栏也仅在聊天页显示，设置等页面全屏 ([12f611a](https://github.com/laochenfei233/Prism-Agent-R/commit/12f611a154ff64d0cdd582fcb9ec12de06332227))
* 交付前修复 — settings_add_provider 加密存 key + TaskRunPanel 运行监控 ([9eed36b](https://github.com/laochenfei233/Prism-Agent-R/commit/9eed36b77233c50d2a3eab4ec7daf477d097d0a5))
* 使用 @tauri-apps/api/core 替代 window.__TAURI__ ([739f274](https://github.com/laochenfei233/Prism-Agent-R/commit/739f274339259488693610e5908947697c4ac9fb))
* 全部 API 参数名改回 snake_case ([7777c14](https://github.com/laochenfei233/Prism-Agent-R/commit/7777c14888464482a73aab0bbc54edf3c62ac4ba))
* 全链路参数名统一为 camelCase ([473b696](https://github.com/laochenfei233/Prism-Agent-R/commit/473b696d4420aa5050bb285afab3d2bd6b587a0c))
* 参数名 camelCase + 拉取模型列表功能 ([407467a](https://github.com/laochenfei233/Prism-Agent-R/commit/407467a573c47876256ff9259667bcdaaaeb73a6))
* 命令名改回 snake_case（Tauri 2.x 不转换） ([9b9d972](https://github.com/laochenfei233/Prism-Agent-R/commit/9b9d972695a501db2591cbb1e93a7dd913fa1d6b))
* 对话流程修复 ([dc6964c](https://github.com/laochenfei233/Prism-Agent-R/commit/dc6964c40e464e73578d81a0a8ffea665db0d583))
* 改进设置页面 UI 和交互 ([0a01bd1](https://github.com/laochenfei233/Prism-Agent-R/commit/0a01bd1dda11f682245cea8aae775eaf1fbba5c6))
* 消除暗色启动闪白（head 内联脚本）+ 全局焦点环特异性 + 补齐既有 token 别名 ([5837250](https://github.com/laochenfei233/Prism-Agent-R/commit/5837250aaae4496c22f67bc372a9127c54ddb01c))
* 添加 try/catch 和 console.log 调试保存按钮 ([296051d](https://github.com/laochenfei233/Prism-Agent-R/commit/296051d98cbfa966999fb77c10e3a212169896be))
* 添加会话创建错误日志 ([cecf5d5](https://github.com/laochenfei233/Prism-Agent-R/commit/cecf5d5a0917780df134b4c9ca2467173d3e7799))
* 用最简单的 inline style 重写配置页，确保按钮可见 ([efaae0f](https://github.com/laochenfei233/Prism-Agent-R/commit/efaae0f69f951808910c9883e5ed5ed7a133d314))
* 简化 UI，三步配置卡片清晰可见 ([fcdb304](https://github.com/laochenfei233/Prism-Agent-R/commit/fcdb30446f4e3482508b4046cd17fd3d744c062c))
* 评审修复 — TTS 代际保护防双音/跳段 + Azure WS 排空读保尾部定稿 + tts 截断测试 + CI 注释 ([19606ef](https://github.com/laochenfei233/Prism-Agent-R/commit/19606ef2ca8591720873f86e789368df66cbabf8))
* 评审修复 — 删 contextualize_document 死代码 + 护栏 configured 补单测 + 数值行即时保存去双写 + 高级组补工作区设置 ([a43f0d5](https://github.com/laochenfei233/Prism-Agent-R/commit/a43f0d5942efd23b011016644ac4c126da77722e))
* 评审修复 — 转写片段按 (meeting_id, index) 幂等落库（022 唯一索引+ON CONFLICT）+ symlink 加固 + rag_eval 复用服务 ([d67f823](https://github.com/laochenfei233/Prism-Agent-R/commit/d67f82393d1e4e59524992422dd39b9237c90b6b))
* 页面添加 overflow-y: auto，按钮不再被切掉 ([c5a67eb](https://github.com/laochenfei233/Prism-Agent-R/commit/c5a67ebb4f44702a5bd04f747141cbe809285eab))

# Changelog

All notable changes to this project are documented here. This file is maintained by [semantic-release](https://semantic-release.gitbook.io/) — versions and entries are generated from commit messages following [Conventional Commits](https://www.conventionalcommits.org/).

## [Unreleased]

### Added
- Repository engineering polish: root `README.md`, `LICENSE` (MIT), `CONTRIBUTING.md`, initial `CHANGELOG.md`
- Frontend quality gates: eslint (flat config), prettier, `.editorconfig`
- Frontend test framework: vitest + @testing-library/svelte with first component tests
- Rust quality gates in CI: `cargo fmt --check`, `cargo clippy -- -D warnings`
- Automated releases: semantic-release (Conventional Commits → version + CHANGELOG + GitHub Release)

## 0.1.0 (2026-08-21)

Snapshot of the codebase before automated release tooling was introduced. Core functionality accumulated through five design phases:

### Agent Core (Phase 1)
- Agent chat loop with streaming responses, markdown + syntax highlighting, thinking blocks
- Multi-provider model support (OpenAI-compatible, Anthropic-compatible, Ollama), per-agent model assignment
- MCP protocol support (stdio/HTTP/SSE), skill system with marketplace
- Agent tool registry (file read/write/list), memory system, workflow engine with templates
- SQLite persistence with versioned migrations, message/session FTS search

### Panels (Phase 2)
- Main dashboard panels, agent sidebar (six tabs), human-in-the-loop tool approvals

### Extensions (Phase 3)
- Wiki + RAG (contextual retrieval, document parsing, traceable citations, multi-dimensional evals)
- Meeting transcription (ASR), translation with FTS history, OCR, glossary
- Reflection mode, goal setting & monitoring, security guardrails, evaluation & monitoring (agent traces, AgentJudge)
- Accessibility (a11y), context compaction

### Autonomous capabilities (Phase 4)
- Web search toolchain (Tavily/Serper/SearxNG) with caching and graceful degradation
- RAG retrieval enhancements: HyDE, RRF multi-path fusion, cliff-cutoff, idempotent import
- Harness engineering: session lifecycle state machine, agent loop (goal/timer/maker-checker), trace grading, session replay

### Production hardening (Phase 5)
- Three-tier token budgets (Global/Crew/Agent) with model fallback chain and degradation policies
- Guardrails: tool-level whitelist/blacklist/approval, trajectory-level checks, filesystem/network/process sandbox
- Observability: structured JSON Lines logger, exception recording with SQLite persistence
- Workflow engine V2 (budget/guardrails/retry integration), auto-orchestration loop (Spec→Plan→Execute→Review)
- Real-time monitor panel (budgets/exceptions/trends, pause/resume/terminate controls)

### Agent capabilities (ongoing)
- Agent capability test suite (`capability_test.rs`): conversation, tool-execution, local-fs read/write/truncation, sandbox path & command enforcement
- Kanban: Agent task management tools + task board merged into agent cards
