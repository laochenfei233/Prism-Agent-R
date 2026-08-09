"""Microsoft Terminology Collection (TBX) → prism-agent 术语 CSV 转换器

用法: python tbx_to_csv.py <zh-CN.xml> <out.csv>
输出列: source_lang,target_lang,source_term,target_term,category
注意: import_csv 用 splitn(5, ',') 解析（无引号转义），故含逗号的词条被过滤。
"""
import csv
import sys
import unicodedata

from lxml import etree


XML = "http://www.w3.org/XML/1998/namespace"


def clean(s: str) -> str:
    return unicodedata.normalize("NFKC", (s or "").strip())


def main() -> int:
    if len(sys.argv) != 3:
        print("用法: python tbx_to_csv.py <input.xml> <out.csv>")
        return 1
    src, dst = sys.argv[1], sys.argv[2]

    tree = etree.parse(src)
    entries = tree.xpath("//termEntry")
    print(f"termEntry 总数: {len(entries)}")

    rows: list[tuple[str, str, str]] = []
    skipped_comma = 0
    for entry in entries:
        langs: dict[str, str] = {}
        for langset in entry.xpath("./langSet"):
            lang = langset.get(f"{{{XML}}}lang")
            terms = langset.xpath(".//term/text()")
            if terms:
                langs[lang] = clean(terms[0])

        en = langs.get("en-US", "")
        zh = langs.get("zh-CN", "")
        if not en or not zh:
            continue
        # import_csv 无引号转义，含逗号会错位 → 过滤
        if "," in en or "," in zh:
            skipped_comma += 1
            continue
        rows.append((en, zh))

    # 去重（按 en 源词；同源词多译文取第一条）
    seen: set[str] = set()
    unique = []
    for en, zh in rows:
        key = en.lower()
        if key in seen:
            continue
        seen.add(key)
        unique.append((en, zh))
    print(f"有效词条: {len(unique)} (含逗号过滤 {skipped_comma})")

    with open(dst, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["source_lang", "target_lang", "source_term", "target_term", "category"])
        for en, zh in unique:
            w.writerow(["en", "zh", en, zh, "微软术语"])
    print(f"已写入: {dst}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
