# 第三方来源与许可说明

贴吧请求参数和字段解析逻辑由 [MediaCrawler](https://github.com/NanmiCoder/MediaCrawler) 的以下实现改写而来：

- `media_platform/tieba/client.py`
- `media_platform/tieba/help.py`

版权归 relakkes@gmail.com 所有。上游项目使用 **NON-COMMERCIAL LEARNING LICENSE 1.1**；完整许可文本见本项目根目录 [`LICENSE`](./LICENSE)。该许可要求非商业学习/研究使用、保留版权和许可声明，并禁止大规模采集或干扰平台运营。

本项目的 Rust、Vue、数据存储与 UI 集成代码为本项目独立实现。所有采集功能仅面向贴吧公开内容，默认限流和数量上限见“设置”页面及 README。
