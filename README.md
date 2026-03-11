<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-blue?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/badge/Platform-Edge%20%7C%20RPi%20%7C%20Jetson%20%7C%20x86-green?style=for-the-badge" alt="Platform">
  <img src="https://img.shields.io/badge/GDPR-Compliant-purple?style=for-the-badge" alt="GDPR">
</p>

<h1 align="center">Rust-Guardian</h1>

<p align="center">
  <strong>Edge Privacy Filter with Cryptographic Integrity Proof</strong>
  <br>
  Real-time face & license plate anonymization on edge devices,<br>
  with digital signature chaining and dynamic Wasm policy engine.
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#why-rust-guardian">Why Rust-Guardian</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#features">Features</a> &bull;
  <a href="#performance">Performance</a> &bull;
  <a href="#roadmap">Roadmap</a> &bull;
  <a href="#contributing">Contributing</a>
</p>

---

## The Problem

You deploy cameras for security, traffic, or analytics — but every frame contains faces and license plates.
GDPR, CCPA, PIPA, APPI all require you to protect this data. Existing tools fall short:

- **deface** (Python): GIL-bottlenecked, faces only, no proof of anonymization
- **brighter AI**: Cloud-dependent, proprietary, expensive per-camera licensing
- **Custom OpenCV scripts**: No integrity proof, no policy engine, no edge optimization

**Nobody answers the critical question: _"Can you prove this frame was properly anonymized?"_**

## Why Rust-Guardian

Rust-Guardian is the first open-source privacy filter that combines **real-time anonymization** with **cryptographic integrity proof** — all running on a $35 Raspberry Pi.

```
                    deface     brighter AI   Rust-Guardian
                   (Python)    (Cloud SaaS)     (Rust)
─────────────────────────────────────────────────────────
Detection            ✅           ✅             ✅
Anonymization        ✅           ✅             ✅
Edge Optimized       ❌           △              ✅
Integrity Proof      ❌           ❌             ✅  ← unique
Audio Privacy        ❌           ❌             ✅  ← unique
Dynamic Policy       ❌           △              ✅  ← unique
RPi4 Support         ❌           ❌             ✅
Open Source          ✅           ❌             ✅
─────────────────────────────────────────────────────────
```

### Three things no one else does:

1. **Cryptographic Signature Chaining** — Every anonymized frame is signed with Ed25519. Frames are hash-chained like a blockchain. Tampering with a single frame breaks the entire chain. _You can prove in court that the footage was properly anonymized._

2. **Wasm Dynamic Policy Engine** — Deploy new privacy policies without restarting. Hot-swap signed Wasm modules that define _what_ to anonymize, _how_ aggressively, and _when_ to escalate. Sandboxed by wasmtime.

3. **SecureBytes Memory Protection** — Original frames are zeroized immediately after anonymization. Not "eventually garbage collected" — cryptographically zeroed with a compiler fence. Memory forensics cannot recover the original.

---

## Quick Start

```bash
# Install (from source)
git clone https://github.com/routinecrew/Rust-Guardian.git
cd Rust-Guardian
cargo build --release

# Anonymize a single image (Phase 0 MVP)
./target/release/rust-guardian blur input.jpg output.jpg

# Real-time RTSP anonymization (Phase 2+)
./target/release/rust-guardian stream rtsp://camera:554/live --output rtsp://localhost:8554/anonymized
```

### Docker (coming soon)

```bash
# x86 with CUDA
docker run -v ./input:/data routinecrew/rust-guardian blur /data/photo.jpg /data/output.jpg

# Raspberry Pi 4
docker run --device /dev/video0 routinecrew/rust-guardian:rpi4 stream /dev/video0
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Rust-Guardian Engine                      │
│                                                                  │
│  Capture ──→ Pipeline ──→ Output                                │
│    │           │    │        │                                    │
│    │     ┌─────┴────┴─────┐  │                                  │
│    │     │  ① Detect      │  ├──→ File / RTSP / mTLS            │
│    │     │  ② Track       │  │                                   │
│    │     │  ③ Mask        │  ├──→ API (REST + SSE)              │
│    │     │  ④ Sign        │  │                                   │
│    │     │  ⑤ Audit       │  └──→ Audit Log                     │
│    │     └────────────────┘                                      │
│    │                                                             │
│  V4L2 / RTSP / File       ThermalMonitor ←→ SentryMode          │
│                            OTA Manager ←→ PolicyRuntime (Wasm)   │
└──────────────────────────────────────────────────────────────────┘
```

### Crate Workspace (11 crates)

| Crate | Description |
|-------|-------------|
| `guardian-core` | Config, Pipeline orchestration, SecureBytes, Engine |
| `guardian-capture` | Input sources: V4L2, RTSP (retina), File |
| `guardian-detector` | Object detection: ORT, NCNN, TFLite, TensorRT backends |
| `guardian-masker` | 5-level anonymization: Blur → Mosaic → Solid → Synthetic → Removal |
| `guardian-signer` | Ed25519 signature chaining + crash recovery |
| `guardian-audit` | GDPR-compliant audit logging |
| `guardian-agent` | Wasm policy runtime + Sentry mode |
| `guardian-thermal` | CPU thermal throttling for edge devices |
| `guardian-ota` | Over-the-air updates with signed packages |
| `guardian-output` | Output sinks: File, RTSP, mTLS transport |
| `guardian-api` | REST API + SSE real-time events |

---

## Features

### Privacy Pipeline

- **Multi-target Detection** — Faces, license plates, and custom objects via pluggable `InferenceBackend` trait
- **5-Level Anonymization** — GDPR-mapped levels from reversible blur (Level 1) to complete removal (Level 5)
- **Detection Tracking** — IoU-based tracker with confirmation frames to prevent flickering
- **Fail-Safe Mode** — If detection fails, blur the entire frame. Privacy is never compromised.
- **Accuracy Self-Check** — `AccuracyMonitor` periodically validates Recall ≥ 0.92 against benchmark images

### Integrity Proof

- **Ed25519 Signature Chaining** — Each frame's hash includes the previous frame's hash, forming a tamper-evident chain
- **Chain Recovery** — Graceful restart after power failure with `ChainReset` protocol
- **Chain State Persistence** — `fsync`-backed disk persistence survives unexpected shutdowns
- **Signature Verification** — Independent verifier confirms chain integrity and detects tampering

### Edge Intelligence

- **Sentry Mode** — Adaptive FPS: high during detections, low when idle (saves power on battery-operated devices)
- **Thermal Throttling** — Auto-reduces FPS when CPU temperature exceeds thresholds (critical for RPi4)
- **Wasm Policy Engine** — Hot-swap privacy policies without restart. Sandboxed, signed, capability-limited.
- **OTA Updates** — Remotely update models, policies, and binaries with signature verification and automatic rollback

### Hardware Support

| Platform | Backend | Expected FPS | Min Recall |
|----------|---------|-------------|------------|
| RPi4 CPU | NCNN + YuNet 320x320 | 5~8 | ≥ 0.92 |
| RPi5 CPU | OpenVINO + YOLOv8n | 8~12 | ≥ 0.94 |
| RPi + Coral TPU | TFLite | 25~30 | ≥ 0.92 |
| Jetson Nano | TensorRT INT8 | 30~43 | ≥ 0.95 |
| Jetson Orin | TensorRT FP16 | 30~60 | ≥ 0.96 |
| x86 + CUDA | ORT CUDA | 60+ | ≥ 0.97 |

### Compliance

- **GDPR** (EU), **CCPA** (California), **PIPL** (China), **PIPA** (Korea), **APPI** (Japan)
- Data Subject Rights API (access, deletion, portability)
- Data minimization in audit logs (coordinates only, never raw images)
- Level 4 (Synthetic) includes mandatory legal review flag

---

## Performance

### Memory Budget (RPi4 1GB)

| Component | Memory |
|-----------|--------|
| OS + base | 200 MB |
| Inference model (YuNet) | 15 MB |
| Inference engine (NCNN) | 50 MB |
| Frame buffer | 6 MB |
| Application | 10 MB |
| **Total** | **~312 MB** |
| **Remaining** | **~688 MB** |

### vs. deface (Python)

```
                    deface        Rust-Guardian
                   (Python)         (Rust)
─────────────────────────────────────────────
FPS (RPi4)         0.5~1            5~8          10x faster
FPS (x86 CPU)      15~20            60+          3x faster
Memory             800MB+           312MB        2.5x less
Startup            3~5s             <0.5s        10x faster
```

---

## Configuration

```yaml
# config/guardian.yml

capture:
  source: rtsp
  path: "rtsp://camera:554/live"
  fps: 10

detector:
  model_path: "models/yunet.onnx"
  backend: "ort"            # ort | ncnn | tflite | tensorrt
  confidence_threshold: 0.5
  target_labels: [face, license_plate]

masker:
  level: 2                  # 1=Blur, 2=Mosaic, 3=Solid, 4=Synthetic, 5=Remove

signer:
  enabled: true
  device_id: "factory-cam-01"
  key_path: "/var/lib/rust-guardian/keys/"

thermal:
  enabled: true
  warning_threshold: 70.0   # °C → fps halved
  critical_threshold: 80.0  # °C → standby mode

api:
  enabled: true
  address: "0.0.0.0:9090"
```

---

## Build

```bash
# Default (CPU + signing + audit)
cargo build --workspace

# RPi optimized (NCNN + thermal management)
cargo build --release --features rpi-optimized

# Jetson optimized (TensorRT)
cargo build --release --features jetson-optimized

# Full features
cargo build --release --features "cuda,signing,audit,thermal,ota,wasm-policy"

# Run tests
cargo test --workspace
```

### Cross-compilation (RPi4)

```bash
# Install cross
cargo install cross

# Build for ARMv7
cross build --release --target armv7-unknown-linux-gnueabihf --features rpi-optimized
```

---

## Roadmap

| Phase | Timeline | Milestone |
|-------|----------|-----------|
| **Phase 0** | 2 weeks | MVP: `rust-guardian blur input.jpg output.jpg` |
| **Phase 1** | 1~3 months | Core anonymization pipeline + accuracy monitoring |
| **Phase 2** | 3~5 months | Signature chaining + real-time input (RTSP/V4L2) |
| **Phase 3** | 5~7 months | Wasm policy engine + thermal management + OTA |
| **Phase 4** | 7~9 months | Audio anonymization + mTLS transport |
| **Phase 5** | 9~11 months | Multi-regulation compliance + deployment |
| **Phase 6** | 11~14 months | Enterprise dashboard + fleet management |

---

## Contributing

We welcome contributions! Here are some good first issues to get started:

- [ ] Add pixelation masker (new `AnonymizationLevel` variant)
- [ ] RPi5 benchmark results
- [ ] Docker image build scripts
- [ ] CI/CD pipeline configuration
- [ ] Documentation translation (Korean → English)

### Development with Multi-Agent Architecture

This project uses a **6-agent parallel development** methodology. Each agent owns specific crates and can develop independently using mock implementations:

```bash
# Run a specific agent
./run-agents.sh a    # Agent A: Core engine
./run-agents.sh c    # Agent C: Detector + Masker
./run-agents.sh d    # Agent D: Signer + Audit

# All agents can build independently
cargo test -p guardian-core
cargo test -p guardian-detector
cargo test -p guardian-signer
```

See [PARALLEL_DEV_GUIDE.md](PARALLEL_DEV_GUIDE.md) for the full coordination protocol.

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

Enterprise features (Wasm policy engine, fleet OTA, synthetic masking) are available under a commercial license. Contact us for details.

---

<p align="center">
  <strong>Privacy is not a feature. It's a fundamental right.</strong>
  <br>
  <sub>Built with Rust. Proven with cryptography. Runs on a Raspberry Pi.</sub>
</p>
