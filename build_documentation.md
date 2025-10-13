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

## 2. Build

```bash
cargo build 
```

