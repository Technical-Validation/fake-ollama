# fakeOllama

## 项目简介

fakeOllama 是一个 Rust 编写的代理服务器，用于模拟 Ollama API 接口，同时将请求转发到兼容 OpenAI API 格式的后端服务（如 OpenAI、Azure OpenAI、本地部署的开源模型等）。这个工具特别适合开发环境中测试和集成 Ollama 客户端应用，而无需运行完整的 Ollama 服务。

## 主要功能

- 模拟 Ollama API 接口，包括 `/api/chat`、`/api/generate`、`/api/tags` 等
- 支持流式响应（streaming）
- 将请求转发到兼容 OpenAI 格式的 API 服务
- 动态配置支持的模型列表
- 完整的请求和响应日志记录

## 安装

确保您已安装 Rust 和 Cargo（推荐 Rust 1.87.0 或更高版本）：

```bash
cargo build --release
```

## 使用方法

### 启动服务器

```bash
cargo run -- --url https://api.openai.com --api-key your-api-key --enabled-models llama2,mistral
```

### 命令行参数

- `--url` 或 `-u`: 远程 API 服务器的 URL（必需）
- `--api-key` 或 `-a`: 远程 API 服务器的 API 密钥（必需）
- `--enabled-models`: 启用的模型列表，以逗号分隔（必需）

### API 端点

fakeOllama 服务器在 `http://127.0.0.1:11434` 上运行，提供以下端点：

- `GET /`: 状态检查
- `POST /api/chat`: Ollama 聊天 API
- `POST /v1/chat/completions`: OpenAI 兼容的聊天补全 API
- `POST /api/generate`: Ollama 文本生成 API
- `GET /api/tags`: 获取可用模型列表

## 示例请求

### 聊天请求

```bash
curl http://127.0.0.1:11434/api/chat \
  -H "Content-Type: application/json" \
  -d '{"model":"llama2","messages":[{"role":"user","content":"Hello, how are you?"}],"stream":true}'
```

### 生成请求

```bash
curl http://127.0.0.1:11434/api/generate \
  -H "Content-Type: application/json" \
  -d '{"model":"mistral","prompt":"Write a short poem about programming","stream":false}'
```

## 开发

### 运行测试

```bash
cargo test
```

### 依赖项

主要依赖项包括：
- reqwest 0.12.21: HTTP 客户端
- axum 0.7.9: Web 框架
- tokio 1.45.1: 异步运行时
- serde 1.0.219 和 serde_json 1.0.140: JSON 序列化/反序列化
- clap 4.5.40: 命令行参数解析
- chrono 0.4.41: 日期和时间处理
- futures-util 0.3.31: 异步工具

## 许可证

[MIT](LICENSE)
