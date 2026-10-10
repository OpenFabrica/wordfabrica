# WordCraft 字体资源

此目录属于 WordCraft 仓库，无独立代码包或服务。`fonts-v*` annotated tag 标记资源版本。

- `manifest.txt`：构建输入及每个内置字体的原始来源、许可证和 SHA-256。六个内置字体文件从 craft-fonts 的 `8dcdacd5153e64560d109541a47d806f26f048c0` 版本原样导入。
- `server/`：运行时扩展字体；目前为 Noto Serif CJK SC Regular，来源为 [Noto CJK](https://github.com/notofonts/noto-cjk/tree/main/Serif/OTF/SimplifiedChinese)，精确文件由 `SHA256SUMS` 固定。
- 每个字体目录的 `OFL.txt`：随字体分发的许可证，不删除。
- `SHA256SUMS`：所有字体文件的校验值。在此目录运行 `shasum -a 256 -c SHA256SUMS` 验证。

详细归属见仓库根目录 `ATTRIBUTION.md`，构建和部署说明见 `docs/web-fonts.md`。
