# 术语库（Glossary）资源

本目录存放可直接导入 prism-agent 翻译术语表的 CSV 文件。
导入格式与 `glossary_import_csv` 一致：

```
source_lang,target_lang,source_term,target_term,category
en,zh,FOB,船上交货,外贸术语
```

> 注意：导入器用 `splitn(5, ',')` 解析且无引号转义，词条内含逗号会错位。
> 本目录所有 CSV 已过滤含逗号的词条，可安全导入。

## 文件清单

| 文件 | 词条数 | 覆盖 | 来源 |
|------|--------|------|------|
| `microsoft_terms_zh-CN.csv` | 33,542 | 微软通用术语（IT/商务/系统 UI） | Microsoft Terminology Collection（官方 TBX 导出，GitHub 镜像 [sumpler/Microsoft-Terminology-Collection-And-Style-Guides](https://github.com/sumpler/Microsoft-Terminology-Collection-And-Style-Guides)） |
| `foreign_trade_terms.csv` | 118 | INCOTERMS 2020 + 外贸单证/结算/物流/报关 | 手工整理（国际商会 INCOTERMS 2020 官方定义） |
| `hs_codes_chapters.csv` | 98 | HS 编码 01-99 章（商品分类） | 世界海关组织 HS 2022 协调制度 |
| `nutrition_supplements.csv` | 165 | 营养成分/剂型/法规/宣称 | 手工整理（DRIs / FDA DSHEA / CODEX） |
| `toys_terms.csv` | 102 | 玩具安全标准（EN71/ASTM F963/GB 6675）+ 分类 | 手工整理（欧盟/美国/中国玩具安全标准） |
| `mechanical_terms.csv` | 214 | 机械设备/零件/材料/工艺/术语 | 手工整理（GB/T 机械工程术语） |
| `ecommerce_terms.csv` | 176 | 电商平台/运营/物流/售后 | 手工整理（亚马逊/阿里巴巴国际站常用词） |

## 使用方法

应用内：设置 → 翻译 → 术语表 → 导入 CSV（选择本目录文件路径）。

## 一键导入（推荐）

内置词库已打包进应用（`src-tauri/resources/glossary/`，构建时经 `bundle.resources`
复制到资源目录）。翻译页 → 术语表面板 →「内置词库一键导入」，点击即可批量导入：

- `glossary_builtin_list` — 列出内置词表（名称/描述/文件）
- `glossary_import_builtin {file}` — 导入指定词表（防路径穿越校验，跳过表头）

打包配置：`src-tauri/tauri.conf.json` → `bundle.resources`：
`"resources/glossary/*.csv": "glossary/"`

新增/更新词表步骤：替换 `src-tauri/resources/glossary/*.csv`（与 `docs/glossary/` 同步）→ 重新构建即可。

或命令行（需提供文件路径）：

```
en,zh 两列固定；category 为可选的第 5 列
```

## 生成脚本

- `tbx_to_csv.py` — 微软术语集 TBX → CSV 转换器（lxml，无第三方依赖）

```bash
python tbx_to_csv.py zh-CN.xml microsoft_terms_zh-CN.csv
```

## 更新说明

- 微软术语集为 2024-10 快照（zh-CN.xml SHA-256: 见 GitHub 仓库）
- INCOTERMS 基于 2020 版（2020-01-01 生效）；HS 编码基于 2022 版
- 玩具安全：EN 71（欧盟）、ASTM F963-23（美国）、GB 6675（中国）
