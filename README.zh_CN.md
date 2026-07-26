# Qubit FS Registry

为 [`qubit-fs`](https://crates.io/crates/qubit-fs) 提供 provider 发现、配置与
SPI registry 集成。

仅使用文件系统 trait 和值类型的程序应只依赖 `qubit-fs`；只有需要运行时
provider 选择时才需要依赖本 crate。
