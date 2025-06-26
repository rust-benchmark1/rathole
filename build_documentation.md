# Installation and Running of rathole

This document provides step-by-step instructions to install required dependencies, build, and run **rathole** on Windows.

---

## 1. Resolving link errors on Windows

The **libgit2-sys** dependency requires Windows security APIs from `advapi32.lib`. To ensure these symbols are linked correctly:

```powershell
cargo clean
cargo update -p libgit2-sys
$Env:RUSTFLAGS = "-l advapi32"
```

This configures the MSVC linker to include the necessary library.

---

## 4. Build

Compile an optimized release binary:

```bash
cargo build --release
```

The executable will be located at:

```
target/release/rathole.exe
```

---

## 5. Run

### 5.1 Server Mode

1. Create a `server.toml` configuration file (minimal example):

   ```toml
   [server]
   bind_addr     = "0.0.0.0:2333"
   default_token = "your_secret_token"

   [server.services.my_nas_ssh]
   token     = "your_secret_token"
   bind_addr = "0.0.0.0:5202"
   ```
2. Start the server process:

   ```powershell
   .\target\release\rathole.exe --server server.toml
   ```

---

### 5.2 Client Mode

1. Create a `client.toml` configuration file:

   ```toml
   [client]
   remote_addr = "your-server.com:2333"

   [client.services.my_nas_ssh]
   token      = "your_secret_token"
   local_addr = "127.0.0.1:22"
   ```
2. Start the client process:

   ```powershell
   .\target\release\rathole.exe --client client.toml
   ```

---
